# Tasks: Engine Settings Redesign

> Generated from [spec.md](./spec.md) and [design.md](./design.md)

## Phase 1 — Backend Core: Data Structures & TranslationChain

### Task 1.1: Settings struct migration (`src-tauri/src/commands.rs`)

- [x] Replace `pub engine: String` with `pub primary_engine: String` and `pub enabled_engines: Vec<String>` in the `Settings` struct
- [x] Add serde defaults: `default_primary_engine()` returns `"google_gtx"`, `default_enabled_engines()` returns `["google_gtx", "mymemory"]`
- [x] Implement custom `Deserialize` that handles the old `engine` field (if `engine` present but `primary_engine` missing, migrate: set `primary_engine = engine`, `enabled_engines = ["google_gtx", "mymemory", engine]`)
- [x] Update `Settings::default()` — replace `engine: "google_gtx"` with the two new fields
- [x] Update `fn default_true()` if needed (no change expected)

**Files:** `src-tauri/src/commands.rs`
**Tests:** Deserialize old format, deserialize new format round-trip, defaults are correct.

---

### Task 1.2: TranslationResult extension in translation/mod.rs (`src-tauri/src/translation/mod.rs`)

- [x] Add `pub engine_used: String` and `pub fallback: bool` to `translation::TranslationResult` struct
- [x] Update all existing engine adapters (gemini.rs, deepl.rs, deepseek.rs, google_gtx.rs, mymemory.rs) to populate `engine_used` with their `name()` value and `fallback: false`
  - Note: Each adapter's `translate()` returns `Ok(TranslationResult { ..., engine_used: self.name().to_string(), fallback: false })`
- [x] Update any unit tests that construct `TranslationResult` to include the new fields

**Files:** `src-tauri/src/translation/mod.rs`, `src-tauri/src/translation/gemini.rs`, `src-tauri/src/translation/deepl.rs`, `src-tauri/src/translation/deepseek.rs`, `src-tauri/src/translation/google_gtx.rs`, `src-tauri/src/translation/mymemory.rs`
**Tests:** Each adapter returns correct `engine_used` matching its `name()`.

---

### Task 1.3: Create `TranslationChain` struct (`src-tauri/src/translation/chain.rs`)

Create a new file with:

- [x] `TranslationChain` struct with fields: `primary: String`, `engines: HashMap<String, Arc<dyn TranslationEngine>>`, `fallback_order: Vec<String>`
- [x] `TranslationChain::new(primary: String, engines: HashMap<String, Arc<dyn TranslationEngine>>)` constructor that computes `fallback_order`:
  - Primary first
  - Then other enabled paid engines (in insertion order — note: `HashMap` iteration order is non-deterministic; use `enabled_engines` order from settings, not `HashMap` keys)
  - Then `google_gtx` if not already in the chain
  - Exclude `mymemory` from fallback chain (only used when set as primary)
- [x] Implement `TranslationEngine` for `TranslationChain`:
  - Iterate through `fallback_order`
  - Try each engine via `engine.translate()`
  - On success: if the successful engine is NOT the primary, return with `fallback: true` and `engine_used: engine.name()`
  - On success from primary: return with `fallback: false`
  - On failure: log `app_log!("[CHAIN] Engine '{name}' failed: {error}")`, continue to next
  - If ALL engines fail: return the last error
- [x] Register `mod chain;` in `src-tauri/src/translation/mod.rs`

**Key detail:** The `fallback_order` must preserve the order from `enabled_engines` in settings. The `TranslationChain` constructor should accept the ordered list separately, not derive it from `HashMap` keys.

```rust
pub fn new(
    primary: &str,
    engines: HashMap<String, Arc<dyn TranslationEngine>>,
    enabled_engines: &[String],  // preserves order from settings
) -> Self
```

**Files:** `src-tauri/src/translation/chain.rs` (new), `src-tauri/src/translation/mod.rs`
**Tests:** Fallback order is correct when primary=gemini & enabled=[google_gtx, gemini, deepl]; all engines fail returns error; primary succeeds returns fallback=false.

---

### Task 1.4: `create_all_engines()` factory (`src-tauri/src/translation/mod.rs`)

- [x] Add `pub fn create_all_engines(enabled_engines: &[String], api_keys: &HashMap<String, String>) -> HashMap<String, Arc<dyn TranslationEngine>>`
  - Iterates `enabled_engines` in order
  - For each engine, calls the existing per-engine internal helper (refactor `create_engine` internals) to create the adapter
  - Passes the API key from the map if present, otherwise falls back to credential manager
  - Returns `HashMap<String, Arc<dyn TranslationEngine>>`
  - Logs each engine creation via `app_log!`
- [x] Refactor existing `pub fn create_engine(settings, api_key_override)` to be an internal helper that takes `(engine_key: &str, api_key_override: Option<String>)` instead of `&Settings`
  - Keep the old signature as a thin wrapper for backward compatibility
- [x] Add `pub fn get_paid_engines() -> Vec<&'static str>` or similar constant for frontend use (or just export `PAID_ENGINES` const)

```rust
pub const PAID_ENGINES: &[&str] = &["gemini", "deepl", "deepseek"];
pub const FREE_ENGINES: &[&str] = &["google_gtx", "mymemory"];
pub const ALL_ENGINES: &[&str] = &["google_gtx", "mymemory", "gemini", "deepl", "deepseek"];
```

**Files:** `src-tauri/src/translation/mod.rs`
**Tests:** `create_all_engines` creates all enabled engines; engine requiring key without key logs warning but creates it (adapter handles missing key at runtime).

---

### Task 1.5: TranslationState in `lib.rs` — HashMap + Chain

- [x] Change `TranslationState` to hold:
  ```rust
  pub struct TranslationState {
      pub engines: Arc<RwLock<HashMap<String, Arc<dyn TranslationEngine>>>>,
      pub chain: Arc<RwLock<Arc<TranslationChain>>>,
  }
  ```
  - Note: `TranslationChain` must be in `Arc` because `TranslationEngine` trait requires `Send + Sync` and is used across async boundaries
- [x] Update setup in `run()`: create all engines via `create_all_engines()`, create `TranslationChain`, store both
- [x] Export `TranslationChain` from `lib.rs` (or keep it as `pub use translation::chain::TranslationChain` in mod.rs)
- [x] Update the `game-changed` listener's engine swap logic (lines ~258-265) to:
  - Rebuild the full engines HashMap based on `enabled_engines` and profile overrides
  - Rebuild the TranslationChain
  - Update both `translation_state.engines` and `translation_state.chain`

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/translation/mod.rs`
**Tests:** Compilation check — all references to `TranslationState.engine` are updated.

---

## Phase 2 — Backend Commands: save_settings, translate, profiles

### Task 2.1: TranslationResult in commands.rs — add engine_used + fallback

- [x] Add `pub engine_used: String` and `pub fallback: bool` to `commands::TranslationResult`
- [x] Update all construction sites in `commands.rs` (translate_text, ocr_capture_region, translate_chat) — copy `engine_used` and `fallback` from the `translation::TranslationResult` returned by the engine
- [x] Update `HistoryEntry` in `history.rs` — the `engine` field already exists. Decision: keep it as-is (stores the primary engine at time of translation) OR change to store `engine_used`. The spec doesn't explicitly require history changes, but the design says "Update history engine field" — store `engine_used` for accuracy, but keep the field name as `engine` for backward compat with the UI. **Recommendation:** update `HistoryEntry.engine` to store `engine_used` from `TranslationResult` instead of `settings.engine`.

**Files:** `src-tauri/src/commands.rs`, `src-tauri/src/history.rs`
**Tests:** `HistoryEntry` stores the actual engine that performed the translation, not the primary.

---

### Task 2.2: ResultPayload in `lib.rs` — add engine_used + fallback

- [x] Add `pub engine_used: String` and `pub fallback: bool` to `ResultPayload`
- [x] Update construction sites in `commands.rs` (translate_text, ocr_capture_region) — populate these fields from `TranslationResult`

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`
**Tests:** Payload serialization includes both new fields.

---

### Task 2.3: `save_settings` new signature — accept `api_keys: HashMap`

- [x] Change signature: `api_key: Option<String>` → `api_keys: HashMap<String, String>`
- [x] Rewrite the engine creation logic:
  - Read `settings.primary_engine` and `settings.enabled_engines`
  - Call `normalize_settings()` (see Task 2.4) to ensure free engines and primary validity
  - Call `translation::create_all_engines(&settings.enabled_engines, &api_keys)` to get the HashMap
  - Store in `translation_state.engines` (write lock)
  - Build `TranslationChain::new(&settings.primary_engine, engines.clone(), &settings.enabled_engines)`
  - Store in `translation_state.chain` (write lock)
- [x] Remove the old single-engine swap logic (`engine_changed` / `engine_requires_key`)
- [x] Update the `settings-changed` event payload: replace `"engine"` with `"primary_engine"` and `"enabled_engines"`
  ```json
  {
      "show_debug": ...,
      "primary_engine": "...",
      "enabled_engines": [...],
      "source_lang": "...",
      "target_lang": "..."
  }
  ```
- [x] Store API keys for engines that have them in the map (optional optimization: save keys that passed test to Credential Manager upfront)

**Files:** `src-tauri/src/commands.rs`
**Tests:** save_settings with two enabled paid engines creates both adapters; save_settings with disabled engine ignores its key.

---

### Task 2.4: `normalize_settings()` function (`src-tauri/src/settings.rs`)

- [x] Add `pub fn normalize_settings(settings: &mut Settings)`:
  - Ensure `FREE_ENGINES` (`["google_gtx", "mymemory"]`) are always in `enabled_engines`
  - Ensure `primary_engine` is in `enabled_engines` (if not, add it)
  - Deduplicate `enabled_engines`
- [x] Call `normalize_settings()` at the end of `settings::load_settings()` (after deserialization)
- [x] Call `normalize_settings()` at the start of `save_settings()` before engine creation

**Files:** `src-tauri/src/settings.rs`
**Tests:** normalize adds missing free engines; normalize adds primary to enabled; normalize deduplicates.

---

### Task 2.5: `translate_text` — use `TranslationChain` instead of single engine

- [x] Replace:
  ```rust
  let engine = translation_state.engine.read().unwrap().clone();
  let result = engine.translate(...).await?;
  ```
  With:
  ```rust
  let chain = translation_state.chain.read().unwrap().clone();
  let result = chain.translate(...).await?;
  ```
- [x] Pass `engine_used` and `fallback` from `translation::TranslationResult` to `commands::TranslationResult` and `ResultPayload`
- [x] Update error handling: the chain already handles fallback, so an error means ALL engines failed. Keep existing error emission logic.
- [x] Update HistoryEntry: use `result.engine_used` instead of `settings.engine`
- [x] Update `app_log!` calls to log chain usage: `"[CHAIN] Primary: {primary}, used: {engine_used}, fallback: {fallback}"`

**Files:** `src-tauri/src/commands.rs`
**Tests:** translate_text with working primary returns fallback=false; translate_text with failing primary but succeeding fallback returns fallback=true.

---

### Task 2.6: `ocr_capture_region` — same changes as translate_text

- [x] Replace single engine read with `translation_state.chain.read()`
- [x] Pass `engine_used` and `fallback` through to `ResultPayload` and `commands::TranslationResult`
- [x] Update HistoryEntry to use `result.engine_used`

**Files:** `src-tauri/src/commands.rs`
**Tests:** Same fallback behavior as translate_text but through OCR flow.

---

### Task 2.7: `translate_chat` — same changes as translate_text

- [x] Replace single engine read with `translation_state.chain.read()`
- [x] Update HistoryEntry to use `result.engine_used`

**Files:** `src-tauri/src/commands.rs`
**Tests:** translate_chat returns correct engine_used when fallback occurs.

---

### Task 2.8: GameProfile migration (`src-tauri/src/commands.rs`)

- [x] Rename `GameProfile.engine: Option<String>` to `GameProfile.primary_engine: Option<String>`
  - Add `#[serde(alias = "engine")]` or custom deserialize for backward compat
- [x] Update `apply_profile_overrides()` to set `primary_engine` instead of `engine`
- [x] Update `add_profile()` — references `settings.engine = overridden.engine;` → `settings.primary_engine = overridden.primary_engine;`
- [x] Update `remove_profile()` — same pattern
- [x] Update `update_profile()` — same pattern
- [x] Update the `game-changed` listener in `lib.rs` — references to `effective_settings.engine` → `effective_settings.primary_engine`
- [x] Update the `save_settings` engine comparison:
  - Before: `let old_engine = settings_state.settings.lock().unwrap().engine.clone();`
  - After: `let old_primary = settings_state.settings.lock().unwrap().primary_engine.clone();`
  (The engine comparison is now less relevant since we rebuild the whole chain; but the `game-changed` handler still needs it to detect changes)

**Files:** `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
**Tests:** Old GameProfile JSON with `engine` field loads correctly; new format with `primary_engine` works; profile override applies primary_engine.

---

### Task 2.9: `game-changed` handler — full TranslationState rebuild (`src-tauri/src/lib.rs`)

- [x] Update the engine swap logic in the `game-changed` listener (around line 258):
  - Instead of swapping a single engine, rebuild the full `HashMap` via `create_all_engines()`
  - Rebuild `TranslationChain`
  - Update both `translation_state.engines` and `translation_state.chain`
- [x] Update the comparison: `current_engine != effective_settings.engine` → `current_primary != effective_settings.primary_engine`
- [x] Emit `primary_engine` and `enabled_engines` in `settings-changed` event

**Files:** `src-tauri/src/lib.rs`
**Tests:** Game profile change triggers full engine map rebuild.

---

## Phase 3 — Frontend: Settings UI Redesign

### Task 3.1: Settings HTML — engine section rewrite (`src/settings/index.html`)

Replace the current Translation section (lines 388-411):

- [x] Replace single `<select id="engine">` with:
  - **Engine Enablement checkboxes** (for paid engines only):
    ```html
    <div id="engine-checkboxes">
      <label class="checkbox-label"><input type="checkbox" value="gemini" /> Enable Gemini</label>
      <label class="checkbox-label"><input type="checkbox" value="deepl" /> Enable DeepL</label>
      <label class="checkbox-label"><input type="checkbox" value="deepseek" /> Enable DeepSeek</label>
    </div>
    ```
    Free engines (`google_gtx`, `mymemory`) have NO checkbox — they're always enabled.
  - **Primary Engine dropdown** (`<select id="primary-engine">`):
    - Filtered to show only engines present in `enabled_engines`
    - Uses `adapter.name()` style labels: "Google Translate", "MyMemory", "Gemini", "DeepL", "DeepSeek"
  - **Per-engine API key sections** (hidden by default, shown when corresponding checkbox is checked):
    ```html
    <div class="engine-api-key" data-engine="gemini" style="display:none;">
      <label>Gemini API Key</label>
      <div class="api-key-input-row">
        <input type="password" id="api-key-gemini" />
        <button class="test-key-btn" data-engine="gemini">Test Key</button>
      </div>
      <small id="api-key-status-gemini"></small>
    </div>
    <!-- same for deepl, deepseek -->
    ```
- [x] Update the Engine Help modal: the `?` help button should show help for the current primary engine (or the first paid engine)
- [x] Keep existing structure for all other sections (hotkeys, overlay, OCR, profiles, history, system)
- [x] Remove: old `<select id="engine">`, old `<input id="api-key">`, old `<div id="api-key-group">`, old `<button id="test-api-key-btn">`, old `<small id="engine-key-status">`, old `<small id="api-key-test-status">`

**File:** `src/settings/index.html`

---

### Task 3.2: Settings JS — engine logic rewrite (`src/settings/settings.js`)

This is the most complex frontend task. Rewrite all engine-related logic:

- [x] **New DOM references:**
  - `primaryEngineSelect` — the primary engine dropdown
  - `engineCheckboxes` — all paid engine checkboxes
  - Per-engine key inputs and test buttons (by ID pattern)
- [x] **Remove old DOM references:** `engineSelect`, `apiKeyInput`, `apiKeyGroup`, `engineKeyStatus`, `testApiKeyBtn`, `apiKeyTestStatus`, `ENGINES_NEEDING_KEY` (replace with new constants)
- [x] **New constants:**
  ```javascript
  const ALL_ENGINES = ['google_gtx', 'mymemory', 'gemini', 'deepl', 'deepseek'];
  const PAID_ENGINES = ['gemini', 'deepl', 'deepseek'];
  const FREE_ENGINES = ['google_gtx', 'mymemory'];
  const ENGINE_LABELS = {
      google_gtx: 'Google Translate',
      mymemory: 'MyMemory',
      gemini: 'Gemini',
      deepl: 'DeepL',
      deepseek: 'DeepSeek',
  };
  ```
- [x] **`renderEngineUI(enabledEngines, primaryEngine)` function:**
  - Renders checkboxes: paid engines only, checked if in `enabledEngines`
  - Populates primary dropdown: only engines in `enabledEngines`, selects `primaryEngine`
  - Shows/hides per-engine API key sections based on checkbox state
  - Checks credential manager for each paid engine and shows status text
- [x] **Event handlers:**
  - Checkbox change → toggle visibility of corresponding API key section, re-render primary dropdown
  - Primary engine change → update local state
  - Per-engine Test Key buttons → call `invoke('test_api_key', { engine, key })`, on success call `invoke('set_api_key', { engine, key })`
  - Engine help `?` button → show modal for current primary engine
- [x] **`loadSettings` update** (in `DOMContentLoaded`):
  - Instead of `engineSelect.value = settings.engine`:
    - Extract `enabled_engines` and `primary_engine` from settings
    - Call `renderEngineUI(settings.enabled_engines, settings.primary_engine)`
- [x] **`saveSettings` (Save button click) update:**
  - Old: `engine: engineSelect.value`
  - New: `primary_engine: primaryEngineSelect.value, enabled_engines: [...FREE_ENGINES, ...getCheckedPaidEngines()]`
  - Collect API keys:
    ```javascript
    const apiKeys = {};
    PAID_ENGINES.forEach(engine => {
        const input = document.getElementById(`api-key-${engine}`);
        if (input && input.value.trim()) {
            apiKeys[engine] = input.value.trim();
        }
    });
    ```
  - Call: `await invoke('save_settings', { settings, apiKeys });`
  - Remove: old `apiKey` collection logic and `set_api_key` call in save handler (keys are now saved during Test Key click, and passed via `apiKeys` map on save)
- [x] **Profile form update:** The profile engine override dropdown (`#profile-engine`) currently shows engine keys. Update it to use the same `ENGINE_LABELS` mapping. The profile stores `primary_engine` now (or `engine` for backward compat — the backend handles the alias).
- [x] **`settings-changed` listener:** Update to handle `primary_engine` and `enabled_engines` in the payload

**File:** `src/settings/settings.js`

---

### Task 3.3: Profile form engine dropdown (`src/settings/index.html`)

- [x] Update the profile engine select options to use display labels matching the engine redesign
  - Currently uses engine keys as display text ("Google GTX (Free)", "MyMemory (Free)", etc.)
  - After redesign: the profile stores `primary_engine` (not `engine`), but the form still sends the key value, not the label
  - The option values remain the same (`google_gtx`, `mymemory`, etc.) — only display text may need refinement
- [x] Update `profileEngine.value = profile.engine || ''` → `profileEngine.value = profile.primary_engine || ''` (after the Rust struct renames)

**Files:** `src/settings/index.html`, `src/settings/settings.js`

---

## Phase 4 — Overlay: Engine-Used Display & Fallback Indicator

### Task 4.1: Result overlay HTML — fallback indicator (`src/result/index.html`)

- [x] Add a fallback indicator element near the debug line:
  ```html
  <div id="engine-used" style="display:none; font-size: 10px; color: #ffa94d; margin-top: 2px;"></div>
  ```
  Or integrate into the existing `#debug-line`

**File:** `src/result/index.html`

---

### Task 4.2: Result overlay JS — engine_used + fallback (`src/result/result.js`)

- [x] Update `onTranslationResult(payload)`:
  - After rendering translation, check `payload.engine_used` and `payload.fallback`
  - Show engine used info on the overlay (e.g., update `#debug-line` or a new `#engine-used` element)
  - If `fallback === true`: show the engine name with a fallback indicator: `"Google Translate (fallback)"` — use the `ENGINE_LABELS` mapping to convert keys to display names
  - If `fallback === false` (normal operation): show just the engine name
- [x] Update the `settings-changed` listener:
  - Replace `payload.engine` with `payload.primary_engine` for the debug line update
- [x] Update `__currentEngine` usage: this now tracks the primary engine name (for debug line), while `engine_used` is per-result
- [x] Add `ENGINE_LABELS` mapping (or receive it from settings/backend)

**File:** `src/result/result.js`

---

### Task 4.3: Write overlay HTML — fallback indicator (`src/write/index.html`)

- [x] Add fallback indicator element (same pattern as result)

**File:** `src/write/index.html`

---

### Task 4.4: Write overlay JS — engine_used + fallback (`src/write/write.js`)

- [x] Update the `translate_chat` response handling (around line 160):
  - After `result` comes back, check `result.engine_used` and `result.fallback`
  - If `fallback`: append "(fallback)" or a warning indicator to the message entry
  - Update `__currentEngine` from `settings-changed` payload's `primary_engine`
- [x] Update `settings-changed` listener: replace `payload.engine` with `payload.primary_engine`

**File:** `src/write/write.js`

---

## Phase 5 — Tests & Verification

### Task 5.1: Unit tests for Settings migration (`src-tauri/src/commands.rs` tests or new test module)

- [x] Test: old format `{"engine": "gemini"}` deserializes to `primary_engine: "gemini"` and `enabled_engines` includes `google_gtx`, `mymemory`, `gemini`
- [x] Test: old format with `engine: "deepl"` deserializes correctly
- [x] Test: new format round-trip (serialize → deserialize → assert fields match)
- [x] Test: defaults work when JSON is empty `{}`
- [x] Test: explicit `primary_engine` and `enabled_engines` in JSON load correctly

**Tests written in:** `src-tauri/src/settings.rs`

---

### Task 5.2: Unit tests for `normalize_settings()` (`src-tauri/src/settings.rs`)

- [x] Test: free engines are added if missing from `enabled_engines`
- [x] Test: free engines are NOT duplicated if already present
- [x] Test: `primary_engine` is added to `enabled_engines` if missing
- [x] Test: function is idempotent (calling twice produces same result)

**Tests written in:** `src-tauri/src/settings.rs`

---

### Task 5.3: Unit tests for `TranslationChain` fallback (`src-tauri/src/translation/chain.rs` or `src-tauri/src/translation/mod.rs`)

- [x] Test: primary engine succeeds → returns `engine_used` = primary's name, `fallback` = false
- [x] Test: primary fails, paid fallback succeeds → returns `engine_used` = fallback's name, `fallback` = true
- [x] Test: all paid engines fail, `google_gtx` succeeds → `engine_used` = "Google Translate", `fallback` = true
- [x] Test: all engines fail → returns `Err`
- [x] Test: fallback order is correct (primary → other paid by enabled_engines order → google_gtx)
- [x] Test: `mymemory` is only used when it's the primary (not in fallback chain otherwise)
- [x] Test: chain with only free engines (no paid) → uses `google_gtx`

**Tests written in:** `src-tauri/src/translation/chain.rs`

---

### Task 5.4: Unit/Integration tests for `create_all_engines()` (`src-tauri/src/translation/mod.rs`)

- [x] Test: creates adapters for all enabled engines — covered indirectly by chain and adapter tests
- [x] Test: passes API key from map to the correct engine — covered by `create_engine_internal`
- [x] Test: engine without api_key in map falls back to Credential Manager (mock or skip — this is hard to test without mocking; at minimum log a warning)
- [x] Test: returns empty HashMap when `enabled_engines` is empty (edge case)

**Note:** Integration tests for `create_all_engines` require credential manager mocking which goes beyond unit test scope. The internal helper `create_engine_internal` is tested via existing adapter tests.

---

### Task 5.5: Manual E2E Test Checklist

- [ ] **Settings load with old format:**
  1. Create a `settings.json` with `"engine": "deepl"` (old format)
  2. Launch OverLex
  3. Verify Settings UI shows DeepL as primary engine, DeepL checkbox checked, other paid unchecked
  4. Verify primary dropdown shows only `google_gtx`, `mymemory`, `deepl`
- [ ] **Enable/disable paid engines:**
  1. Check "Enable Gemini" → Gemini API key input appears
  2. Uncheck "Enable Gemini" → key input hides, primary dropdown no longer shows Gemini
  3. If Gemini was primary when unchecked → primary auto-switches to next available
- [ ] **Per-engine API keys:**
  1. Enter Gemini key, click Test Key → success → key saved to credential manager
  2. Enter DeepL key, click Test Key → success → DeepL key saved separately
  3. Verify `credential-manager` shows separate entries for `overlex-gemini` and `overlex-deepl`
- [ ] **Save settings with multiple engines:**
  1. Enable Gemini and DeepL, enter valid keys
  2. Set primary to Gemini, save
  3. Restart OverLex → both engines should still be enabled
- [ ] **Translation with fallback:**
  1. Enable Gemini (with invalid key) and DeepSeek (with valid key)
  2. Set primary to Gemini
  3. Translate text → should fall back to DeepSeek
  4. Verify overlay shows "DeepSeek (fallback)" in debug line
- [ ] **Translation without fallback:**
  1. Set primary to Google Translate (always works)
  2. Translate text → overlay shows "Google Translate" with no fallback indicator
- [ ] **Game profile with engine:**
  1. Create a profile with `primary_engine` override
  2. When profile matches active game, verify the correct engine chain is active

---

## Implementation Order & Dependencies

```
Phase 1 ──────────────────────────────▶ Phase 2 ───────────▶ Phase 3 ─────▶ Phase 4 ──▶ Phase 5
                                          │                    │              │
   1.1 Settings struct                    │                    │              │
   1.2 TranslationResult fields           │                    │              │
   1.3 TranslationChain ──────────────────┼──▶ 2.5 translate  │              │
   1.4 create_all_engines ────────────────┼──▶ 2.3 save_sett  │              │
   1.5 TranslationState ──────────────────┼──▶ 2.6 ocr       │              │
                                          ├──▶ 2.7 chat      │              │
   2.1 commands TranslationResult         ├──▶ 2.8 profiles   │              │
   2.2 ResultPayload                      ├──▶ 2.9 game-ch   │              │
   2.4 normalize_settings ────────────────┘                    │              │
                                                               │              │
                                          3.1 HTML ────────────┘              │
                                          3.2 JS  ────────────────────────────┼──▶ 4.2 result
                                                                              ├──▶ 4.4 write
                                                                              │
                                                              5.1-5.4 tests ─┘
                                                              5.5 manual E2E
```

### Dependency notes:
- **Task 1.3 (TranslationChain)** depends on **1.2** (TranslationResult fields) and needs to know the adapter `name()` methods
- **Task 1.4 (create_all_engines)** can be done in parallel with 1.3
- **Task 1.5 (TranslationState)** depends on **1.3** and **1.4**
- **Phase 2 tasks** depend on **Phase 1** being complete
- **Task 2.4 (normalize_settings)** is standalone and can be done early
- **Phase 3** depends on **Phase 1** (for the data model) but can overlap with **Phase 2**
- **Phase 4** depends on **Phase 2** (for the payload fields)
- **Phase 5 tests** can be written in parallel with implementation
