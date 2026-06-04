# Tasks: Gemini + DeepL Translation with Game Context

## Phase 1: Trait & Infrastructure (`mod.rs`)

- [x] 1.1 Add `TranslationContext` struct (process_name, profile_name) to `src-tauri/src/translation/mod.rs`
- [x] 1.2 Update `TranslationEngine::translate()` signature: add `context: Option<&TranslationContext>` as 4th param
- [x] 1.3 Add `mod gemini;` and `mod deepl;` declarations; re-export `GeminiAdapter`, `DeepLAdapter`
- [x] 1.4 Update `create_engine()`: add `"gemini"` and `"deepl"` match arms with `settings::get_api_key()`; fix LibreTranslate arm to read API key from keyring instead of `None`

## Phase 2: Update Existing Adapters

- [x] 2.1 `src-tauri/src/translation/google_gtx.rs` — Add `context: Option<&TranslationContext>` param to `translate()`, ignore it
- [x] 2.2 `src-tauri/src/translation/mymemory.rs` — Add `context: Option<&TranslationContext>` param to `translate()`, ignore it
- [x] 2.3 `src-tauri/src/translation/libretranslate.rs` — Add `context: Option<&TranslationContext>` param to `translate()`, ignore it; no other changes needed (API key fix already in 1.4)

## Phase 3: New Adapters

- [x] 3.1 Create `src-tauri/src/translation/gemini.rs` — `GeminiAdapter` struct with `api_key: Option<String>`, `client: Client`; `new()`; `translate()` POST to `gemini-2.0-flash:generateContent` with system instruction (with/without game context); map 429→RateLimit, 5xx→ServiceDown, 400+auth→InvalidApiKey; timeout 15s; `name()` returns `"Gemini"`; `requires_api_key()` returns `true`
- [x] 3.2 Create `src-tauri/src/translation/deepl.rs` — `DeepLAdapter` struct with `api_key: Option<String>`, `client: Client`; `new()`; `translate()` POST to `api-free.deepl.com/v2/translate` with `Authorization: DeepL-Auth-Key` header and optional `context` field; map 429/456→RateLimit, 403→InvalidApiKey, 5xx→ServiceDown; timeout 15s; `name()` returns `"DeepL"`; `requires_api_key()` returns `true`

## Phase 4: Command Integration (`commands.rs`)

- [x] 4.1 `translate_text` — Add `active_game_state: State<ActiveGameState>` param; build `TranslationContext` from `info.process_name` / `info.matched_profile`; pass `context.as_ref()` to `engine.translate()`; handle `InvalidApiKey` → emit `INVALID_API_KEY` error event
- [x] 4.2 `ocr_capture_region` — Same context-building pattern as 4.1; pass context to engine translate call; handle `InvalidApiKey`
- [x] 4.3 `translate_chat` — Same context-building pattern; pass context; handle `InvalidApiKey`

## Phase 5: Settings UI

- [x] 5.1 `src/settings/index.html` — Add `<option value="gemini">Gemini (Requires API Key)</option>` and `<option value="deepl">DeepL (Requires API Key)</option>` to both `#engine` and `#profile-engine` dropdowns
- [x] 5.2 `src/settings/settings.js` — Verified: `get_api_key` / `set_api_key` invoke calls already use engine name dynamically; label updated to indicate API key requirement

## Phase 6: Tests

- [x] 6.1 Unit test: `TranslationContext` construction — verify `None` when no game info, `Some` with values when game active
- [x] 6.2 Unit test: `GeminiAdapter::new()` — assert `name() == "Gemini"`, `requires_api_key() == true`
- [x] 6.3 Unit test: `DeepLAdapter::new()` — assert `name() == "DeepL"`, `requires_api_key() == true`
- [x] 6.4 Unit test: `create_engine()` dispatch — mock `Settings`, assert correct engine type for `"gemini"`, `"deepl"`, `"google_gtx"`, `"mymemory"`, `"libretranslate"`
- [x] 6.5 Unit test: Gemini system instruction — verify context fields present/absent produce correct prompt
- [ ] 6.6 Integration test (ignored): Gemini translate with context — mark `#[ignore]`, requires real API key (deferred: needs Windows + API key)
- [ ] 6.7 Integration test (ignored): DeepL translate with context — mark `#[ignore]`, requires real API key (deferred: needs Windows + API key)

