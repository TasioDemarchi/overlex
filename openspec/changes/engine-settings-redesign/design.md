# Design: Engine Settings Redesign

## Technical Approach

Replace the single-engine architecture with a multi-engine fallback chain. The core idea: `Settings` gains `primary_engine` and `enabled_engines` (backward-compatible via custom deserialization), `TranslationState` holds all enabled engines in a `HashMap`, and a new `TranslationChain` wrapper implements `TranslationEngine` to provide adaptive fallback (primary → other enabled paid engines → `google_gtx`). Translation results carry `engine_used` and `fallback` fields through to overlays. The UI moves from a single dropdown + shared API key to per-engine checkboxes with isolated key inputs.

## Architecture Decisions

### Decision: Settings struct migration strategy

**Choice**: Custom `Deserialize` with `#[serde(default)]` + migration in `load_settings()`
**Alternatives considered**: (A) Separate migration function called on load, (B) Serde `default` + post-load normalization
**Rationale**: Option B is cleaner — Serde handles the happy path (new format) and `Default` fills missing fields. After deserialization, a normalization step ensures `enabled_engines` always contains free engines and that `primary_engine` is in `enabled_engines`. This avoids a separate migration pass and keeps the struct Serde-idiomatic.

### Decision: TranslationChain as impl TranslationEngine vs. separate orchestration in commands

**Choice**: `TranslationChain` struct implements `TranslationEngine`
**Alternatives considered**: (A) Fallback logic inline in `translate_text`/`ocr_capture_region`, (B) Separate `translate_with_fallback()` free function
**Rationale**: Implementing `TranslationEngine` means `TranslationChain` is a drop-in replacement — commands call `.translate()` the same way. The chain encapsulates the retry logic, engine selection, and result annotation. This preserves the existing `TranslationState.engine` field type (`Arc<dyn TranslationEngine>`) and minimizes command-level changes.

### Decision: Per-engine API keys in save_settings

**Choice**: `save_settings` accepts `api_keys: HashMap<String, String>` (engine→key map) alongside `Settings`
**Alternatives considered**: (A) Keep single `api_key` param, (B) Frontend calls `set_api_key` per engine before `save_settings`
**Rationale**: Option B creates race conditions and N+1 calls. A single map is atomic — all keys arrive together, all engines are created in one pass. The existing per-engine `set_api_key`/`get_api_key` commands remain for the "Test Key" button but the main save flow uses the map.

### Decision: Engine name display mapping

**Choice**: Hardcoded map in `TranslationChain` (e.g., `"gemini"` → `"Gemini"`)
**Alternatives considered**: (A) `name()` method on each adapter (already exists), (B) Frontend-only mapping
**Rationale**: Each adapter already has `name()` returning a human-readable string (e.g., `"Google Translate"`, `"Gemini"`, `"DeepL"`). Use that directly — no new mapping needed.

## Data Flow

### Translation with fallback

```
translate_text() / ocr_capture_region()
    │
    ├─ Read settings.primary_engine + settings.enabled_engines
    │
    ├─ translation_state.chain.translate(text, src, tgt, ctx)
    │       │
    │       ├─ Try primary engine → success? return result + engine_used + fallback=false
    │       │
    │       ├─ On error: try next enabled paid engine (in enabled_engines order)
    │       │   → success? return result + engine_used + fallback=true
    │       │
    │       └─ All paid fail → try google_gtx
    │           → success? return result + engine_used + fallback=true
    │           → fail? return error
    │
    ├─ Build ResultPayload { …, engine_used, fallback }
    │
    └─ emit_result() → overlay shows engine name + fallback indicator
```

### Settings save flow

```
Frontend save_settings:
    │
    ├─ Collect: Settings{primary_engine, enabled_engines, …}
    ├─ Collect: api_keys = { gemini: "...", deepseek: "..." }
    │
    └─ invoke('save_settings', { settings, apiKeys })
            │
            ├─ Validate hotkeys
            ├─ save_settings_to_disk(&settings)
            ├─ Normalize: ensure free engines in enabled_engines
            ├─ For each engine in enabled_engines:
            │     create_engine(engine, api_keys.get(engine))
            │     → store in TranslationState.engines HashMap
            ├─ Rebuild TranslationChain from primary + enabled_engines
            │     → store in TranslationState.chain
            ├─ Re-register hotkeys
            └─ emit("settings-changed", { primary_engine, enabled_engines, … })
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src-tauri/src/commands.rs` | Modify | Replace `engine: String` with `primary_engine` + `enabled_engines` in `Settings`. Add custom deserialization migration. Add `engine_used`/`fallback` to `TranslationResult` and `ResultPayload`. Update `save_settings`, `translate_text`, `ocr_capture_region`, `translate_chat` to use chain. Remove `api_key` param from `save_settings`, add `api_keys: HashMap`. Update `GameProfile.engine` to `primary_engine`. Update history `engine` field. |
| `src-tauri/src/lib.rs` | Modify | Change `TranslationState` to hold `engines: Arc<RwLock<HashMap<String, Arc<dyn TranslationEngine>>>>` and `chain: Arc<RwLock<Arc<TranslationChain>>>`. Update setup to initialize chain. Update `ResultPayload` struct. Update engine swap in `game-changed` listener. |
| `src-tauri/src/translation/mod.rs` | Modify | Add `TranslationChain` struct implementing `TranslationEngine`. Add `create_all_engines()` factory. Keep `create_engine()` as internal helper. |
| `src-tauri/src/translation/chain.rs` | Create | `TranslationChain` with adaptive fallback logic: primary → other enabled paid → google_gtx. Returns `ChainResult` with `engine_used` and `fallback`. |
| `src-tauri/src/settings.rs` | Modify | Add `normalize_settings()` to ensure free engines are always in `enabled_engines` and `primary_engine` is valid. |
| `src/settings/index.html` | Modify | Replace single engine dropdown + API key input with: engine checkboxes for paid engines, primary engine dropdown filtered to enabled, per-engine API key sections that show/hide on toggle. |
| `src/settings/settings.js` | Modify | Major rewrite of engine UI: render engine checkboxes, dynamic primary dropdown, per-engine API key inputs and Test buttons, `save_settings` sends `api_keys` map instead of single key. |
| `src/result/result.js` | Modify | Listen to `translation-result` payload for `engine_used` and `fallback`. Update debug line to show actual engine + fallback indicator. |
| `src/write/write.js` | Modify | Same as result.js — update debug line with `engine_used` and `fallback`. |

## Interfaces / Contracts

### Settings struct (Rust)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    // …existing fields unchanged…
    
    /// Primary translation engine (must be in enabled_engines)
    #[serde(default = "default_primary_engine")]
    pub primary_engine: String,
    
    /// All enabled engines (free engines always included)
    #[serde(default = "default_enabled_engines")]
    pub enabled_engines: Vec<String>,
    
    // REMOVED: pub engine: String
    
    // …existing fields unchanged…
}

fn default_primary_engine() -> String { "google_gtx".to_string() }
fn default_enabled_engines() -> Vec<String> { vec!["google_gtx".into(), "mymemory".into()] }

// Custom Deserialize to migrate old format:
// - If "engine" present but "primary_engine" missing: migrate
// - If "primary_engine" missing and "engine" missing: use defaults
impl<'de> Deserialize<'de> for Settings {
    // Custom impl that handles backward compatibility
}
```

### Migration strategy

```rust
// In load_settings() or a normalize function:
fn normalize_settings(settings: &mut Settings) {
    // Ensure free engines are always present
    const FREE_ENGINES: &[&str] = &["google_gtx", "mymemory"];
    for &engine in FREE_ENGINES {
        if !settings.enabled_engines.contains(&engine.to_string()) {
            settings.enabled_engines.push(engine.to_string());
        }
    }
    // Ensure primary_engine is in enabled_engines
    if !settings.enabled_engines.contains(&settings.primary_engine) {
        // If primary not enabled, add it (user explicitly chose it)
        settings.enabled_engines.push(settings.primary_engine.clone());
    }
}
```

### TranslationChain (new struct)

```rust
pub struct TranslationChain {
    primary: String,
    engines: HashMap<String, Arc<dyn TranslationEngine>>,
    /// Ordered list: primary first, then other paid enabled, then google_gtx
    fallback_order: Vec<String>,
}

impl TranslationChain {
    pub fn new(
        primary: String,
        engines: HashMap<String, Arc<dyn TranslationEngine>>,
    ) -> Self {
        let paid_engines: Vec<String> = engines.keys()
            .filter(|k| k != &"google_gtx" && k != &"mymemory" && k != &primary)
            .cloned()
            .collect();
        
        let mut fallback_order = vec![primary.clone()];
        fallback_order.extend(paid_engines);
        if !fallback_order.contains(&"google_gtx".to_string()) {
            fallback_order.push("google_gtx".to_string());
        }
        
        Self { primary, engines, fallback_order }
    }
}

pub struct ChainResult {
    pub original: String,
    pub translated: String,
    pub detected_source: Option<String>,
    pub engine_used: String,
    pub fallback: bool,
}

#[async_trait]
impl TranslationEngine for TranslationChain {
    async fn translate(
        &self, text: &str, source: &str, target: &str,
        context: Option<&TranslationContext>,
    ) -> Result<TranslationResult, TranslationError> {
        // Try each engine in fallback_order
        // On success from non-primary, return with engine_used + fallback
        // Uses internal ChainResult internally, but returns TranslationResult
        // (engine_used/fallback returned via separate method or wrapper)
    }
}
```

### ResultPayload extension

```rust
#[derive(serde::Serialize, Clone)]
pub struct ResultPayload {
    pub original: String,
    pub translated: String,
    pub error: Option<String>,
    pub timeout_ms: u32,
    pub source_lang: String,
    pub target_lang: String,
    pub engine_used: String,      // NEW
    pub fallback: bool,            // NEW
}
```

### TranslationResult extension (commands.rs)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationResult {
    pub original: String,
    pub translated: String,
    pub detected_source: Option<String>,
    pub engine_used: String,      // NEW
    pub fallback: bool,            // NEW
}
```

### save_settings new signature

```rust
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    api_keys: HashMap<String, String>,  // NEW: per-engine keys
    settings_state: tauri::State<'_, SettingsState>,
    // …other state…
) -> Result<(), String>
```

### Frontend data sent to save_settings

```javascript
// New: collect API keys per engine
const apiKeys = {};
ENGINES_NEEDING_KEY.forEach(engine => {
    const input = document.getElementById(`api-key-${engine}`);
    if (input && input.value.trim()) {
        apiKeys[engine] = input.value.trim();
    }
});

await invoke('save_settings', {
    settings: { /* …primary_engine, enabled_engines… */ },
    apiKeys
});
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `normalize_settings()` — free engines always present, primary in enabled | Direct function call assertions |
| Unit | `TranslationChain` fallback order — primary first, then paid, then google_gtx | Mock engines that fail/succeed in specific patterns |
| Unit | `TranslationChain` — no infinite loop when all fail | Verify it returns error after exhausting all engines |
| Unit | Settings deserialization — old format `{ "engine": "gemini" }` migrates | Deserialize JSON string and assert `primary_engine` + `enabled_engines` |
| Unit | Settings deserialization — new format round-trips | Serialize then deserialize, verify all fields |
| Integration | `save_settings` creates all enabled engines, not just primary | Call with multi-engine config, check `TranslationState.engines` map |
| Integration | Fallback produces `fallback: true` and correct `engine_used` | Make primary fail, verify result metadata |
| E2E (manual) | Frontend: checkbox toggles show/hide API key inputs | Visual verification |
| E2E (manual) | Frontend: primary dropdown filters to enabled engines only | Visual verification |
| E2E (manual) | Translation with primary failure falls back, overlay shows engine change | Manual test with invalid API key |

## Migration / Rollout

**Settings JSON migration**: On `load_settings()`, if the JSON contains `engine` but not `primary_engine`, the custom deserializer will:
1. Set `primary_engine = engine`
2. Set `enabled_engines = ["google_gtx", "mymemory", engine]` (all free + the chosen engine)
3. The old `engine` field is ignored on re-serialization (it's not in the struct)

**Rollback**: Revert to the commit before this change. The old `engine` field is preserved in existing settings files until the first save with the new code — at which point it's replaced by `primary_engine`/`enabled_engines`. If rolled back, the old code reads `engine` which no longer exists and falls back to default (`google_gtx`).

**Mitigation**: On first save, the new code writes ONLY `primary_engine` and `enabled_engines`. To support rollback, add a one-time migration that writes both `engine` (for backward compat) AND the new fields during the transition period. This can be removed after one release cycle.

## Open Questions

- [ ] Should `mymemory` appear in the primary engine dropdown or only `google_gtx` as the free default? (Spec says free engines are "always enabled" but doesn't restrict them from being primary)
- [ ] Profile `engine` override should map to `primary_engine` — need to update `GameProfile` struct and `apply_profile_overrides` as well
- [ ] Confirm: should the overlay fallback indicator say "Google Translate (fallback)" or "Google GTX (fallback)" — use adapter `name()` or the settings key?