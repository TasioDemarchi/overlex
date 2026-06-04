# Design: Gemini + DeepL Translation with Game Context

## Technical Approach

Add two AI-powered translation engines (Gemini 2.0 Flash, DeepL Free) that consume game context for better gaming translations. The core change is extending the `TranslationEngine` trait with a `TranslationContext` parameter, implementing both adapters, wiring context from `ActiveGameState` into all three translation commands, and updating the settings UI with new dropdown options.

## Architecture Decisions

### Decision: TranslationContext Struct Location

**Choice**: Define `TranslationContext` in `src-tauri/src/translation/mod.rs` alongside the trait
**Alternatives considered**: Separate `context.rs` file
**Rationale**: The struct is 2 fields, tightly coupled to the trait. A separate file adds indirection for no cohesion gain. Keep it with the trait definition.

### Decision: Trait Signature Change Strategy

**Choice**: Option A — add `context: Option<&TranslationContext>` as 4th param to `translate()`, update all existing adapters to accept and ignore it
**Alternatives considered**: Option B (new method `translate_with_context` with default impl), Option C (setter on adapter)
**Rationale**: Simplest and compiler-enforced. All four call sites and three adapters get updated in one pass. No vtable bloat or forgotten overrides. A default-impl method is fragile — new adapters might forget to override it.

### Decision: Missing API Key Handling

**Choice**: Adapters store `Option<String>` for API key; `create_engine()` passes `None` if key missing; `translate()` returns `InvalidApiKey` error at call time
**Alternatives considered**: Return Google GTX fallback from `create_engine()` when key missing
**Rationale**: Failing silently with a different engine masks the user's intent. The spec requires `InvalidApiKey` error — clear feedback is better than silent degradation. The UI already shows this error overlay.

### Decision: create_engine() Signature

**Choice**: Keep `create_engine(settings: &Settings) -> Box<dyn TranslationEngine>`. Read API keys internally via `settings::get_api_key()`.
**Alternatives considered**: Change signature to return Result, pass keys as params
**Rationale**: Minimal disruption. `get_api_key()` already exists and uses keyring. All three call sites (setup, save_settings, game-changed) continue working unchanged. Adapters that need keys get `Some(key)`, others get `None`.

### Decision: DeepL Free Only

**Choice**: Hardcode `api-free.deepl.com` endpoint
**Alternatives considered**: Configurable free/pro toggle
**Rationale**: Spec explicitly scopes to free tier only. Adding a toggle is scope creep — Phase 2 is context-aware translation, not DeepL feature parity.

## Data Flow

Context flows from foreground process detection through commands to adapters:

```
game_detection thread ──→ ActiveGameState.info { process_name, matched_profile }
                              │
                              ▼
translate_text / ocr_capture_region / translate_chat
  1. Lock ActiveGameState.info
  2. Build Option<TranslationContext>
  3. engine.translate(text, src, tgt, ctx.as_ref())
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
       GeminiAdapter    DeepLAdapter    GoogleGtx/MyMemory/Libre
       (injects prompt) (context param) (ignores context)
```

API key retrieval happens at engine creation time:

```
create_engine(&settings)
  ├─ "gemini"  → get_api_key("gemini") → GeminiAdapter::new(key)
  ├─ "deepl"   → get_api_key("deepl")  → DeepLAdapter::new(key)
  └─ fallback  → GoogleGtxAdapter::new()  (no key needed)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src-tauri/src/translation/mod.rs` | Modify | Add `TranslationContext` struct, update trait signature with 4th param, add gemini/deepl modules and `create_engine` cases |
| `src-tauri/src/translation/gemini.rs` | Create | Gemini 2.0 Flash adapter with systemInstruction context |
| `src-tauri/src/translation/deepl.rs` | Create | DeepL Free adapter with context parameter |
| `src-tauri/src/translation/google_gtx.rs` | Modify | Update `translate()` signature to accept `Option<&TranslationContext>` (ignore it) |
| `src-tauri/src/translation/mymemory.rs` | Modify | Update `translate()` signature to accept `Option<&TranslationContext>` (ignore it) |
| `src-tauri/src/translation/libretranslate.rs` | Modify | Update `translate()` signature to accept `Option<&TranslationContext>` (ignore it) |
| `src-tauri/src/commands.rs` | Modify | Add `ActiveGameState` param to `translate_text`, `translate_chat`, `ocr_capture_region`; build context and pass to engine |
| `src/settings/index.html` | Modify | Add `<option value="gemini">` and `<option value="deepl">` to global engine and profile engine dropdowns |
| `src/settings/settings.js` | Modify | Update API key label logic for Gemini/DeepL, load/save keys per engine |

## Interfaces / Contracts

### TranslationContext

```rust
// src-tauri/src/translation/mod.rs
pub struct TranslationContext {
    pub process_name: Option<String>,
    pub profile_name: Option<String>,
}
```

### Updated Trait Signature

```rust
#[async_trait]
pub trait TranslationEngine: Send + Sync {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
        context: Option<&TranslationContext>,
    ) -> Result<TranslationResult, TranslationError>;

    fn name(&self) -> &str;
    fn requires_api_key(&self) -> bool;
}
```

### GeminiAdapter

```rust
// src-tauri/src/translation/gemini.rs
pub struct GeminiAdapter {
    api_key: Option<String>,
    client: Client,
}

impl GeminiAdapter {
    pub fn new(api_key: Option<String>) -> Self { ... }
}
```

- POST `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}`
- System instruction with game context (see below)
- Body: `{ "systemInstruction": { "parts": [{"text": ...}] }, "contents": [{"parts": [{"text": text}]}], "generationConfig": {"temperature": 0.1} }`
- Response: `response["candidates"][0]["content"]["parts"][0]["text"]`
- `detected_source` always `None`
- 429 → RateLimit, 5xx → ServiceDown, 400 → check for API key error → InvalidApiKey
- Timeout: 15s

**System instruction template**:
- With context: `"You are a professional game translator. Translate the following text from {source} to {target}. Game context: {process_name} (profile: {profile_name}). Respond ONLY with the translated text, no explanations, no quotes, no markdown."`
- Without context: `"You are a professional translator. Translate the following text from {source} to {target}. Respond ONLY with the translated text, no explanations, no quotes, no markdown."`

### DeepLAdapter

```rust
// src-tauri/src/translation/deepl.rs
pub struct DeepLAdapter {
    api_key: Option<String>,
    client: Client,
}

impl DeepLAdapter {
    pub fn new(api_key: Option<String>) -> Self { ... }
}
```

- POST `https://api-free.deepl.com/v2/translate`
- Auth header: `Authorization: DeepL-Auth-Key {api_key}`
- Body: `{ "text": [text], "source_lang": source (optional), "target_lang": target, "context": "Game: {process_name}" (optional) }`
- Response: `response["translations"][0]["text"]`, `detected_source_language` mapped to `detected_source`
- 429/456 → RateLimit, 403 → InvalidApiKey, 5xx → ServiceDown
- Timeout: 15s

### Updated create_engine

```rust
pub fn create_engine(settings: &Settings) -> Box<dyn TranslationEngine> {
    match settings.engine.as_str() {
        "gemini" => {
            let api_key = settings::get_api_key("gemini").ok();
            Box::new(GeminiAdapter::new(api_key))
        }
        "deepl" => {
            let api_key = settings::get_api_key("deepl").ok();
            Box::new(DeepLAdapter::new(api_key))
        }
        "mymemory" => {
            Box::new(MyMemoryAdapter::new())
        }
        "libretranslate" => {
            let api_key = settings::get_api_key("libretranslate").ok();
            Box::new(LibreTranslateAdapter::new(
                settings.libre_translate_url.clone(),
                api_key,
            ))
        }
        "google_gtx" | _ => {
            Box::new(GoogleGtxAdapter::new())
        }
    }
}
```

Note: LibreTranslate currently passes `None` for API key. This change also fixes that — it now reads the key from credential store when set.

### Context Building in Commands

All three translation commands add `active_game_state: tauri::State<'_, ActiveGameState>` and build context:

```rust
let context = {
    let info = active_game_state.info.lock().unwrap();
    match (&info.process_name, &info.matched_profile) {
        (None, None) => None,
        _ => Some(TranslationContext {
            process_name: info.process_name.clone(),
            profile_name: info.matched_profile.clone(),
        }),
    }
};
// Then: engine.translate(&text, &source, &target, context.as_ref()).await
```

### Error Handling for InvalidApiKey

In all three translate commands, when the engine returns `TranslationError::InvalidApiKey`, emit an `overlex-error` event with code `"INVALID_API_KEY"` instead of the generic `"NETWORK_ERROR"`:

```rust
Err(TranslationError::InvalidApiKey) => {
    emit_error(&app_handle, ErrorPayload {
        code: "INVALID_API_KEY".to_string(),
        message: format!("API key required for {} engine. Set it in Settings.", engine_name),
    }, true);
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `TranslationContext` construction | Direct assertion: None when no game, Some with values when game active |
| Unit | `GeminiAdapter::new()` creation | Assert name, requires_api_key |
| Unit | `DeepLAdapter::new()` creation | Assert name, requires_api_key |
| Unit | `create_engine()` dispatch | Mock Settings, assert correct engine type for each string |
| Unit | Gemini system instruction formatting | Assert context fields present/absent produce correct prompt |
| Unit | DeepL request body construction | Assert context field present/absent per TranslationContext |
| Integration | Error response mapping | Mock HTTP responses (429, 403, 456, 5xx) → correct TranslationError variant |
| Integration | Full translate with context | Skip without API keys; mark `#[ignore]` for CI |
| E2E | Settings UI dropdown rendering | Manual verification: Gemini and DeepL options visible |

**Key testing constraint**: Gemini and DeepL require real API keys. Integration tests against live APIs are `#[ignore]` by default. Unit tests cover construction, error mapping, and context formatting without network calls.

## Migration / Rollout

No migration required. Settings JSON with `engine: "google_gtx"` (or any unrecognized value) falls through to the default adapter. The trait signature change compiles across all adapters in one pass. Existing API keys for LibreTranslate continue working unchanged. Users who never set a Gemini/DeepL key never see an error — they only hit `InvalidApiKey` if they select one of those engines.

## Open Questions

- [ ] Should the `systemInstruction` for Gemini include language names (e.g., "English" → "Spanish") instead of codes (e.g., "en" → "es")? The current plan uses codes. Human-readable names might improve Gemini's prompt adherence.