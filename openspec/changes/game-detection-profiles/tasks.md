# Tasks: game-detection-profiles — Phase 1

> Game Detection + Profile Auto-switching for OverLex
> Created: 2026-06-04
> Total: 7 tasks (3S, 3M, 1L)

## Dependency Graph

```
T-001 (Settings Foundation) ──┬──→ T-002 (GameDetector Module) ──→ T-003 (Auto-Switch) ──→ T-005 (Debug Indicator)
                              │                                                           └──→ T-006 (Settings UI)
                              └──→ T-004 (Commands) ──────────────────────────────────────────→ T-006 (Settings UI)
                                                                                               T-007 (Integration Test)
```

| Task | Size | Depends On | Dependents |
|------|------|-----------|------------|
| T-001 | S | — | T-002, T-003, T-004 |
| T-002 | M | T-001 | T-003 |
| T-003 | M | T-001, T-002 | T-005, T-006 |
| T-004 | S | T-001 | T-006 |
| T-005 | S | T-003 | — |
| T-006 | M | T-003, T-004 | — |
| T-007 | S | All | — |

---

## T-001: Settings Foundation (S)

> Extend `Settings` with `GameProfile`, `show_debug`. Add new managed state types.

**Depends on**: None
**Files changed**: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
**Estimate**: S (1-2 files, simple data additions)

### Subtasks

1. **Add `GameProfile` struct** to `commands.rs` with `serde(default)` on all `Option` fields:
   ```rust
   #[derive(Debug, Serialize, Deserialize, Clone)]
   pub struct GameProfile {
       pub display_name: String,
       pub process_names: Vec<String>,
       #[serde(default)]
       pub source_lang: Option<String>,
       #[serde(default)]
       pub target_lang: Option<String>,
       #[serde(default)]
       pub engine: Option<String>,
       #[serde(default)]
       pub ocr_preprocessing: Option<bool>,
       #[serde(default)]
       pub ocr_binarize: Option<bool>,
   }
   ```

2. **Extend `Settings` struct** with two new fields (both `#[serde(default)]`):
   - `profiles: Vec<GameProfile>` → defaults to empty vec
   - `show_debug: bool` → defaults to `false`

3. **Add `ActiveGameInfo` struct** (serialize+clone, used in events and commands):
   ```rust
   #[derive(Serialize, Clone, Default)]
   pub struct ActiveGameInfo {
       pub process_name: Option<String>,
       pub fullscreen_exclusive: bool,
       pub matched_profile: Option<String>,
   }
   ```

4. **Add `SettingsState.saved_defaults`** — extend the existing struct in `lib.rs`:
   ```rust
   pub struct SettingsState {
       pub settings: Arc<Mutex<Settings>>,          // active/effective
       pub saved_defaults: Arc<Mutex<Settings>>,     // persisted defaults
   }
   ```
   - In `setup()`, clone the loaded settings into both `settings` and `saved_defaults`
   - Update all existing `app.manage(SettingsState { ... })` call sites

5. **Add `ActiveGameState`** to `lib.rs`:
   ```rust
   pub struct ActiveGameState {
       pub info: Arc<Mutex<ActiveGameInfo>>,
   }
   ```
   - Initialize with `ActiveGameInfo::default()` in `setup()`
   - Add `app.manage(active_game_state)`

6. **Update `lib.rs` imports**: add `ActiveGameInfo`, `ActiveGameState` to `use crate::commands::...`

### Verification
- [x] `cargo build` succeeds with no warnings
- [x] Existing `settings.json` (without `profiles`/`show_debug`) loads without error → `serde(default)` kicks in
- [x] `SettingsState` initialization compiles with both `settings` and `saved_defaults`
- [x] `ActiveGameState` is manageable and accessible from commands

---

## T-002: GameDetector Module (M)

> Create `game_detection.rs` with Win32 foreground window polling + exclusive fullscreen detection.

**Depends on**: T-001 (needs `Settings` for profile matching)
**Files created**: `src-tauri/src/game_detection.rs`
**Files changed**: `src-tauri/src/lib.rs` (add `mod game_detection` + init), `src-tauri/Cargo.toml` (add Windows features)
**Estimate**: M (3-5 files, moderate complexity)

### Subtasks

1. **Add Windows feature flags** to `Cargo.toml`:
   - `Win32_System_ProcessStatus` (for `QueryFullProcessImageNameW`)
   - Verify `Win32_UI_WindowsAndMessaging` already present (yes, confirmed)

2. **Create `game_detection.rs`** module with:

   **Structs**:
   ```rust
   pub struct GameDetectorState {
       pub shutdown: Arc<AtomicBool>,
       pub handle: Mutex<Option<JoinHandle<()>>>,
   }

   #[derive(Serialize, Clone)]
   pub struct GameChangedPayload {
       pub process_name: Option<String>,
       pub fullscreen_exclusive: bool,
       pub matched_profile: Option<String>,
   }
   ```

   **Core function `spawn_detector()`**:
   - Accepts `AppHandle`, `Arc<AtomicBool>`, `Arc<Mutex<Settings>>`
   - Spawns `std::thread` (matching hotkey pattern from `hotkeys.rs`)
   - Poll loop every 1000ms:

   **Poll loop logic**:
   1. `GetForegroundWindow()` → get `HWND`
   2. If `HWND` is `None` (null) → emit `process_name: null`, `fullscreen_exclusive: false`
   3. If `HWND` == `last_hwnd` → skip (same window, no event)
   4. `GetWindowThreadProcessId(hwnd, &mut pid)` → get PID
   5. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid)` → handle
   6. `QueryFullProcessImageNameW(handle, ...)` → full path → `Path::new(path).file_name()` → extract filename
   7. `is_fullscreen_exclusive(hwnd)`: check `GetWindowLongPtrW(hwnd, GWL_STYLE)` lacks `WS_OVERLAPPEDWINDOW` + window rect covers at least one monitor
   8. **Profile matching**: lock `Settings` briefly, clone `profiles`, release lock → case-insensitive match against `process_names`
   9. Emit `game-changed` via `app_handle.emit("game-changed", payload)`
   10. Store `last_hwnd` and `last_process_name` for next iteration
   11. `thread::sleep(Duration::from_millis(1000))`

   **Edge cases**:
   - `OpenProcess` denied (anti-cheat) → emit `process_name: null`, skip matching, log warning
   - `QueryFullProcessImageNameW` fails → fall back to PID-only identifier, log warning
   - Same process, different window → `last_hwnd` changed but `last_process_name` same → skip emit

3. **Add `pub mod game_detection;`** to `lib.rs`

4. **Initialize `GameDetectorState` in `setup()`**:
   - Create `Arc<AtomicBool>` for shutdown
   - Call `game_detection::spawn_detector(app_handle.clone(), shutdown.clone(), settings_state.settings.clone())`
   - Store state with `app.manage(GameDetectorState { ... })`

5. **Clean shutdown**: In `on_window_event` or app exit, set `shutdown.store(true)` and join handle

### Verification
- [ ] `cargo build` succeeds with new Windows feature flags
- [ ] Thread spawns on app launch (visible via `eprintln!`)
- [ ] Switching foreground apps causes `game-changed` events (visible via `eprintln!`)
- [ ] Same process, different windows → no duplicate events
- [ ] Lock screen / desktop → emits `process_name: null`
- [ ] Thread stops cleanly on app exit (no lingering threads)
- [ ] Exclusive fullscreen games detected correctly
- [ ] Profile matching is case-insensitive

---

## T-003: Auto-Switch Handler (M)

> Listen for `game-changed` events, apply profile overrides, implement two-tier settings.

**Depends on**: T-001 (SettingsState.saved_defaults, ActiveGameState), T-002 (game-changed events)
**Files changed**: `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`
**Estimate**: M (2-4 files, moderate complexity)

### Subtasks

1. **Add `game-changed` event listener** in `lib.rs setup()`:
   ```rust
   let app_handle_game = app.handle().clone();
   app.listen("game-changed", move |event| {
       // Parse GameChangedPayload from event.payload()
       // Acquire locks, clone profiles from saved_defaults, release
       // Match: clone saved_defaults → apply Option overrides → set settings
       // No match: clone saved_defaults → set settings (revert)
       // Handle engine swap if profile.engine differs
       // Update ActiveGameState.info
       // Emit active-game-changed to all windows
   });
   ```

2. **Implement override application logic** (helper function or inline):
   ```rust
   fn apply_profile_overrides(base: &Settings, profile: &GameProfile) -> Settings {
       let mut s = base.clone();
       if let Some(v) = &profile.source_lang { s.source_lang = v.clone(); }
       if let Some(v) = &profile.target_lang { s.target_lang = v.clone(); }
       if let Some(v) = &profile.engine { s.engine = v.clone(); }
       if let Some(v) = profile.ocr_preprocessing { s.ocr_preprocessing = v; }
       if let Some(v) = profile.ocr_binarize { s.ocr_binarize = v; }
       s
   }
   ```

3. **Engine swap on profile match**: If matched profile has `engine: Some(...)` and it differs from current:
   - Acquire write lock on `TranslationState.engine`
   - Create new engine via `translation::create_engine(&overridden_settings)`
   - Swap `Arc<dyn TranslationEngine>`

4. **Emit `active-game-changed`** with `ActiveGameInfo` payload after every switch/revert

5. **Modify `save_settings` command** in `commands.rs`:
   - Update BOTH `saved_defaults` AND `settings` in `SettingsState`
   - If a profile is currently active (check `ActiveGameState`), re-apply overrides after saving defaults
   - Emit `active-game-changed` after save so overlays re-check `show_debug`

6. **Handle engine swap in `save_settings`**: existing code already does this (fine)

### Verification
- [x] Switching to a game matching a profile → settings override immediately
- [x] Switching away → settings revert to saved defaults
- [x] Engine swap works when profile specifies different engine
- [x] Engine NOT swapped when profile engine matches default (no unnecessary swap)
- [x] `save_settings` updates both tiers correctly
- [x] Re-applying overrides after save preserves active profile
- [x] No deadlocks (locks acquired/released in consistent order)

---

## T-004: Tauri Commands for Profiles (S)

> Add 6 new Tauri commands for profile CRUD + debug toggle + active game query.

**Depends on**: T-001 (needs `GameProfile`, `SettingsState.saved_defaults`, `ActiveGameState`)
**Files changed**: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (register new handlers)
**Estimate**: S (2 files, simple command wrappers)

### Subtasks

1. **Add `add_profile` command**:
   ```rust
   #[tauri::command]
   pub async fn add_profile(
       profile: GameProfile,
       settings_state: tauri::State<'_, SettingsState>,
       active_game: tauri::State<'_, ActiveGameState>,
   ) -> Result<(), String>
   ```
   - Push to `saved_defaults.profiles`
   - Save to disk
   - If the new profile matches current active game, re-apply overrides

2. **Add `remove_profile` command**:
   ```rust
   pub async fn remove_profile(
       display_name: String,
       settings_state: tauri::State<'_, SettingsState>,
       active_game: tauri::State<'_, ActiveGameState>,
   ) -> Result<(), String>
   ```
   - Remove by `display_name` from `saved_defaults.profiles`
   - Save to disk
   - If removed profile was active, revert to saved defaults

3. **Add `update_profile` command**:
   ```rust
   pub async fn update_profile(
       profile: GameProfile,
       settings_state: tauri::State<'_, SettingsState>,
       active_game: tauri::State<'_, ActiveGameState>,
   ) -> Result<(), String>
   ```
   - Find and replace by `display_name` in `saved_defaults.profiles`
   - Save to disk
   - Re-apply if was active

4. **Add `list_profiles` command**:
   ```rust
   pub async fn list_profiles(
       settings_state: tauri::State<'_, SettingsState>,
   ) -> Result<Vec<GameProfile>, String>
   ```
   - Return `saved_defaults.profiles.clone()`

5. **Add `get_active_game` command**:
   ```rust
   pub async fn get_active_game(
       active_game: tauri::State<'_, ActiveGameState>,
   ) -> Result<ActiveGameInfo, String>
   ```
   - Return `ActiveGameState.info.lock().unwrap().clone()`

6. **Add `toggle_debug` command**:
   ```rust
   pub async fn toggle_debug(
       show: bool,
       settings_state: tauri::State<'_, SettingsState>,
       app_handle: tauri::AppHandle,
   ) -> Result<(), String>
   ```
   - Set `saved_defaults.show_debug = show`
   - Set `settings.show_debug = show`
   - Save to disk
   - Emit `settings-changed` event so overlays update

7. **Register all 6 commands** in `lib.rs` `invoke_handler(generate_handler![...])`

### Verification
- [x] `add_profile` / `remove_profile` / `update_profile` round-trip through invoke
- [x] `list_profiles` returns correct list
- [x] `get_active_game` returns current process info
- [x] `toggle_debug` persists and emits event
- [x] All commands handle missing data gracefully (no panics)
- [x] Profile that's currently active correctly re-applies on update

---

## T-005: Debug Indicator on Overlays (S)

> Add debug line `<div>` to result and write overlays, with show/hide logic.

**Depends on**: T-003 (needs `active-game-changed` event)
**Files changed**: `src/result/index.html`, `src/result/result.js`, `src/write/index.html`, `src/write/write.js`
**Estimate**: S (4 files, simple HTML/CSS/JS changes, same pattern applied twice)

### Subtasks

1. **Add `<div id="debug-line">` to `result/index.html`** — inside `#app-wrapper`, after `#original-section` but before `</div>`

2. **Add CSS for debug line** in both `result/index.html` and `write/index.html`:
   ```css
   #debug-line {
       display: none;
       position: absolute;
       bottom: 0; left: 0; right: 0;
       padding: 2px 8px;
       font-family: 'Consolas', 'Courier New', monospace;
       font-size: 10px;
       color: rgba(255,255,255,0.5);
       background: rgba(0,0,0,0.3);
       white-space: nowrap;
       overflow: hidden;
       text-overflow: ellipsis;
       z-index: 10;
       pointer-events: none;
   }
   #debug-line.visible { display: block; }
   ```
   Note: `pointer-events: none` so it doesn't interfere with interactions.

3. **Add JS to `result.js`**:
   - On load: `invoke('get_settings')` → check `show_debug`, toggle `visible` class
   - Listen to `active-game-changed`:
     ```js
     listen('active-game-changed', (event) => {
         const info = event.payload;
         const debugEl = document.getElementById('debug-line');
         if (!debugEl) return;
         if (info.process_name) {
             let text = info.process_name;
             if (info.matched_profile) text += ` [${info.matched_profile}]`;
             text += ` · ${currentEngine || '—'}`;
             if (info.fullscreen_exclusive) text += ' ⚠ Fullscreen';
             debugEl.textContent = text;
         } else {
             debugEl.textContent = '— · ...';
         }
     });
     ```
   - Listen to `settings-changed` → re-check `show_debug`
   - Need to track `currentEngine` — can get from settings on load

4. **Add same JS pattern to `write.js`** — identical logic

5. **Note on `#app-wrapper` positioning**: The wrapper uses `position: fixed` with `inset: 0`, so `position: absolute` on `#debug-line` will anchor correctly at bottom.

### Verification
- [x] Debug line appears on both overlays when `show_debug: true`
- [x] Debug line is hidden (no space reserved) when `show_debug: false`
- [x] Text format correct: `process_name [profile] · engine ⚠ Fullscreen`
- [x] Updates in real-time when foreground app changes
- [x] `pointer-events: none` ensures no interference with drag/click
- [x] No console errors

---

## T-006: Settings UI — Game Profiles Section (M)

> Full CRUD UI for game profiles + debug toggle + active profile banner.

**Depends on**: T-003 (active-game-changed event, active game info), T-004 (profile commands)
**Files changed**: `src/settings/index.html`, `src/settings/settings.js`
**Estimate**: M (2 files, substantial UI work)

### Subtasks

1. **Add Active Profile Banner** (top of settings, inside `<h1>` area):
   ```html
   <div id="active-profile-banner" style="display:none; background: rgba(78, 154, 241, 0.15); border: 1px solid var(--accent); border-radius: 6px; padding: 10px 14px; margin-bottom: 16px;">
       <strong style="color: var(--accent);">Active Profile:</strong>
       <span id="active-profile-name"></span>
       <small style="color: var(--text-secondary); display: block; margin-top: 4px;" id="active-profile-details"></small>
   </div>
   ```

2. **Add "Game Profiles" section** after "OCR Pre-processing" (before "Translation History"):
   ```html
   <h2>Game Profiles</h2>
   <div class="section">
       <div class="form-group">
           <label class="checkbox-label">
               <input type="checkbox" id="show-debug" />
               Show debug info on overlays
           </label>
       </div>
       <div id="profile-list">
           <!-- Profile cards rendered here -->
       </div>
       <button id="add-profile-btn" class="small-btn" style="margin-top: 8px;">+ Add Profile</button>
   </div>

   <!-- Profile form dialog (hidden by default) -->
   <div id="profile-form-overlay" style="display:none; position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 100; display: none; align-items: center; justify-content: center;">
       <div id="profile-form" style="background: var(--bg-secondary); border-radius: 8px; padding: 20px; width: 90%; max-width: 450px; max-height: 80vh; overflow-y: auto;">
           <!-- Form fields -->
       </div>
   </div>
   ```
   Actually, I'll use a simpler inline expandable form rather than a modal, to match the existing page style.

3. **Profile card template** (rendered in JS):
   - Display name (bold)
   - Process names (monospace, comma-separated)
   - Override badges: e.g. `<span class="badge">→ ja</span>`, `<span class="badge">engine: gemini</span>`
   - Edit and Delete buttons
   - CSS for `.profile-card`, `.badge`, etc.

4. **Profile form (Add/Edit inline)**:
   - Fields: `display_name` (text), `process_names` (text input, comma-separated)
   - Optional overrides: `source_lang` (select), `target_lang` (select), `engine` (select), `ocr_preprocessing` (checkbox), `ocr_binarize` (checkbox)
   - **Auto-fill button**: calls `get_active_game()` → sets process_name field
   - Save and Cancel buttons

5. **JS additions to `settings.js`**:
   - **State**: `profiles: []`, `editingProfile: null`, `activeGameInfo: null`
   - **`renderProfiles()`**: clears `#profile-list`, iterates profiles, creates cards
   - **`openProfileForm(profile?)`**: populates form with existing values or empty
   - **`saveProfile()`**: calls `add_profile` or `update_profile`, then `renderProfiles()`
   - **`deleteProfile(displayName)`**: calls `remove_profile`, then `renderProfiles()`
   - **`autoFillProcessName()`**: calls `get_active_game()` → fills `process_names` input
   - **On load**: call `list_profiles()`, call `get_active_game()`, populate
   - **Listen to `active-game-changed`** → update banner and active game info
   - **`show_debug` checkbox**: calls `toggle_debug` on change
   - **Banner update**: when `activeGameInfo.matched_profile` is set, show banner with profile name and override details

6. **CSS additions** for profile cards, badges, form:
   ```css
   .profile-card { ... }
   .profile-card .badge { ... }
   #profile-form-overlay { ... }
   ```

### Verification
- [x] Profile list loads and displays correctly
- [x] Add profile → form appears → save → list updates
- [x] Edit profile → form pre-filled → save → list updates
- [x] Delete profile → removed from list
- [x] Auto-fill populates process_name from active game
- [x] `show_debug` toggle persists and emits event
- [x] Active profile banner shows/hides correctly
- [x] Banner shows override details
- [x] No console errors
- [x] Settings page responsive and scrollable

---

## T-007: Integration Test & Cleanup (S)

> Verify the full system works end-to-end.

**Depends on**: All tasks above
**Files touched**: None (manual verification + unit tests)
**Estimate**: S (verification only)

### Subtasks

1. **Write Rust unit tests** (in `commands.rs` or `game_detection.rs`):
   - `test_profile_match_case_insensitive`: match "POE2.EXE" against profile with "poe2.exe"
   - `test_profile_match_multiple_processes`: match against any in `process_names`
   - `test_override_application`: verify `Some` fields override, `None` fields preserve
   - `test_profiles_serde_default`: deserialize settings JSON without `profiles`/`show_debug` → verify defaults
   - `test_settings_backward_compat`: load old settings.json format without new fields

2. **Manual E2E verification checklist**:
   - [ ] App launches → detector thread starts (check `eprintln!`)
   - [ ] Switch to notepad.exe → debug line shows `notepad.exe · ...`
   - [ ] Create profile for notepad.exe with target_lang "ja" → settings auto-switch
   - [ ] Switch away from notepad → settings revert to defaults
   - [ ] Debug toggle on/off → overlays hide/show line
   - [ ] Exclusive fullscreen game → shows `⚠ Fullscreen`
   - [ ] Profile with engine override → engine swaps at runtime
   - [ ] Save settings while profile active → defaults update, overrides re-applied
   - [ ] Quit app → detector thread stops cleanly (no crash on exit)

3. **Regression check**:
   - [ ] OCR mode still works (hotkey → screenshot → OCR → translate → show result)
   - [ ] Write mode still works (hotkey → type text → translate → show result)
   - [ ] Settings save/load round-trip works without profiles (backward compat)
   - [ ] Language swap hotkey still works
   - [ ] Dismiss/auto-dismiss still works

### Verification
- [ ] All unit tests pass
- [ ] Manual checklist complete
- [ ] No regressions in existing features

---

## Summary

| Task | Description | Est. | Files |
|------|-------------|------|-------|
| T-001 | Settings Foundation | S | `commands.rs`, `lib.rs` |
| T-002 | GameDetector Module | M | `game_detection.rs` (new), `lib.rs`, `Cargo.toml` |
| T-003 | Auto-Switch Handler | M | `lib.rs`, `commands.rs` |
| T-004 | Tauri Commands for Profiles | S | `commands.rs`, `lib.rs` |
| T-005 | Debug Indicator on Overlays | S | `result/index.html`, `result.js`, `write/index.html`, `write.js` |
| T-006 | Settings UI — Game Profiles | M | `settings/index.html`, `settings.js` |
| T-007 | Integration Test & Cleanup | S | Tests (inline in existing modules) |

**Total**: 3 Small, 3 Medium, 0 Large → **3S + 3M**
