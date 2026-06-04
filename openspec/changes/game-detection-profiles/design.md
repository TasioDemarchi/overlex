# Design: game-detection-profiles

## Overview

Spawn a lightweight `GameDetector` background thread that polls the foreground window process name every ~1s via Win32 APIs. On process change, emit a `game-changed` Tauri event. If the process matches a user-configured `GameProfile`, apply language/engine/OCR overrides instantly; revert to saved defaults on non-match. A toggleable debug indicator line appears at the bottom of existing result and write overlays. Settings UI adds a Game Profiles section with CRUD and auto-fill from the current foreground process.

## Architecture Decisions

| Decision | Choice | Alternatives | Rationale |
|----------|--------|--------------|-----------|
| Thread model | Dedicated `std::thread` with `AtomicBool` shutdown | Tokio interval task, Tauri async command | Matches existing hotkey thread pattern; detached poll loop is simpler than async interval |
| Process name extraction | `QueryFullProcessImageNameW` with `PROCESS_QUERY_LIMITED_INFORMATION` | `GetModuleFileNameExW` requiring `PROCESS_VM_READ` | Lower privilege; safer for anti-cheat; fewer access rights |
| Fullscreen detection | Window style + rect comparison | `SHQueryUserNotificationState` (requires `Win32_UI_Shell`) | No new Windows feature crate entries; style+rect is sufficient for detection. Shell API noted as future enhancement |
| Settings state model | Two-tier: `settings` (active/effective) + `saved_defaults` (persisted defaults) | Single settings with "diff" tracking, profile override bitmask | Simple clone-and-apply approach; easy to revert by cloning saved_defaults; no diff math |
| Debug indicator | Line `<div>` in existing overlays | Separate webview window | Spec REQ-05 explicitly says "line at bottom of result and write overlays"; no new window needed |
| Profile ID | String (display_name slug) | UUID, numeric ID | Simpler; display_name is unique-enough for a local desktop app; avoids UUID dependency |

## Module Design

### GameDetector (`game_detection.rs`)

```rust
pub struct GameDetectorState {
    pub shutdown: Arc<AtomicBool>,
    pub handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Serialize, Clone)]
pub struct GameChangedPayload {
    pub process_name: Option<String>,      // "poe2.exe" or null
    pub fullscreen_exclusive: bool,
    pub matched_profile: Option<String>,   // display_name or null
}
```

**Thread lifecycle**: Spawn in `setup()`, stored in `ManagedState<GameDetectorState>`. On shutdown, `AtomicBool` signals exit; thread loop breaks and `JoinHandle` is joined.

**Poll loop** (every 1000ms):
1. `GetForegroundWindow()` → if same HWND as last poll, skip (no change)
2. `GetWindowThreadProcessId(hwnd, &mut pid)` → get PID
3. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid)` → handle
4. `QueryFullProcessImageNameW(handle, ...)` → full path → extract filename
5. `is_fullscreen_exclusive(hwnd)` → check style lacks `WS_OVERLAPPEDWINDOW` + rect covers screen
6. If process_name changed from last poll: match against `Settings.profiles` (case-insensitive)
7. Emit `game-changed` via `app_handle.emit()`
8. `thread::sleep(Duration::from_millis(1000))`

**Edge cases**: null HWND → emit `process_name: null`; `OpenProcess` denied → process_name from PID only (lower fidelity, but graceful); same process, different window → skip (no event).

### GameProfile & Settings Extension

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameProfile {
    pub display_name: String,  // also used as profile identifier
    pub process_names: Vec<String>,      // ["poe2.exe", "PathOfExile2.exe"]
    #[serde(default)]
    pub source_lang: Option<String>,     // override, None = use default
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

Added to `Settings`:
```rust
#[serde(default)]
pub profiles: Vec<GameProfile>,
#[serde(default)]
pub show_debug: bool,
```

`serde(default)` ensures backward compatibility: old `settings.json` files load with empty `profiles` and `show_debug: false`.

### Auto-Switch Mechanism

**State changes**:

```rust
pub struct SettingsState {
    pub settings: Arc<Mutex<Settings>>,        // Active/effective settings
    pub saved_defaults: Arc<Mutex<Settings>>,   // Persisted defaults
}

pub struct ActiveGameState {
    pub info: Arc<Mutex<ActiveGameInfo>>,
}

#[derive(Serialize, Clone, Default)]
pub struct ActiveGameInfo {
    pub process_name: Option<String>,
    pub fullscreen_exclusive: bool,
    pub matched_profile: Option<String>,
}
```

**On `game-changed` event** (listener in `lib.rs setup()`):
1. Update `ActiveGameState.info` with new process/profile data
2. Find matching profile via case-insensitive `process_names` comparison
3. **If match**: clone `saved_defaults`, apply profile's `Option` overrides (only `Some` fields), set `settings` to overridden clone. Swap translation engine if `profile.engine` differs from default
4. **If no match**: set `settings` to clone of `saved_defaults`. Swap engine back if needed
5. Emit `active-game-changed` to all windows with `ActiveGameInfo` payload (for debug indicator + settings UI update)

**On `save_settings` command**: update both `saved_defaults` and `settings`. If profile is active, immediately re-apply overrides on top.

### Debug Indicator

**HTML** — inserted as last child of `#app-wrapper` in both `result/index.html` and `write/index.html`:
```html
<div id="debug-line"></div>
```

**CSS** (added to both overlays):
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
}
#debug-line.visible { display: block; }
```

**JS** (added to both `result.js` and `write.js`):
- On load: `invoke('get_settings')` → check `show_debug`, set class `visible`/hidden
- Listen to `active-game-changed` → update text: `{process_name || '—'} · {engine}{fullscreen_exclusive ? ' ⚠ Fullscreen' : ''}`
- Listen to `settings-changed` (from `save_settings`) → re-check `show_debug`

When `show_debug: false`, the element has no space reserved (`display: none`).

### Settings UI

New section **"Game Profiles"** added after "Translation" in `settings/index.html`:

- **`show_debug` checkbox** at top of section
- **Profile list**: rendered as cards showing display name, process names, and override badges (e.g. "→ ja", "engine: gemini")
- **Add/Edit form**: inline form with fields for display_name, process_names (comma-separated), and optional overrides (source_lang, target_lang, engine, ocr_preprocessing, ocr_binarize)
- **Auto-fill button**: calls `get_active_game()` and populates process_name field with current foreground process
- **Delete button**: on each profile card

### Tauri Commands

| Command | Signature | Behavior |
|---------|-----------|----------|
| `add_profile` | `(profile: GameProfile, state: SettingsState)` | Push to `profiles`, save, re-apply if active |
| `remove_profile` | `(display_name: String, state: SettingsState)` | Remove by display_name, save, revert if was active |
| `update_profile` | `(profile: GameProfile, state: SettingsState)` | Replace by display_name, save, re-apply if active |
| `list_profiles` | `(state: SettingsState)` | Return `Vec<GameProfile>` from settings |
| `get_active_game` | `(state: ActiveGameState)` | Return `ActiveGameInfo` |
| `toggle_debug` | `(show: bool, state: SettingsState)` | Set `show_debug`, save, emit event |

## Data Flow

```
┌──────────────┐  poll   ┌────────────────┐
│ GameDetector │──1s───→│ GetForeground  │
│   thread     │        │    Window       │
└──────┬───────┘        └────────────────┘
       │ emit game-changed
       ▼
┌──────────────┐  match  ┌────────────────┐
│  lib.rs      │────────→│ Apply Profile  │
│  handler     │ revert  │ Overrides      │
└──────┬───────┘←────────└────────────────┘
       │ emit active-game-changed
       ▼
┌──────────────┐  ┌──────────────┐
│ result overlay│  │ write overlay │  ← debug-line updates
│  (result.js) │  │  (write.js)   │
└──────────────┘  └──────────────┘
       │
       ▼
┌──────────────┐
│ settings.js  │  ← profile CRUD + show_debug toggle
└──────────────┘
```

## Thread Safety

| State | Type | Access Pattern |
|-------|------|---------------|
| `SettingsState.settings` | `Arc<Mutex<Settings>>` | Write from main thread (game-changed handler, save_settings); read from commands |
| `SettingsState.saved_defaults` | `Arc<Mutex<Settings>>` | Same as settings; cloned for override application |
| `ActiveGameState.info` | `Arc<Mutex<ActiveGameInfo>>` | Write from game-changed handler; read from `get_active_game` command |
| `TranslationState.engine` | `Arc<RwLock<Arc<dyn TranslationEngine>>>` | Existing pattern; write lock only on engine swap |
| `GameDetectorState.shutdown` | `Arc<AtomicBool>` | Set from main thread on app exit; read from detector thread |

GameDetector thread only reads `Settings.profiles` briefly (clone + release lock) during matching. All mutations happen on the main thread via Tauri event dispatch.

## Error Handling

| Failure | Handling |
|---------|----------|
| `OpenProcess` denied | Emit `process_name: null`, `matched_profile: null`; skip matching |
| `QueryFullProcessImageNameW` fails | Fall back to PID-only identifier; log warning |
| No foreground window | Emit `process_name: null`, `fullscreen_exclusive: false` |
| Corrupt `settings.json` (no `profiles`/`show_debug`) | `serde(default)` handles: empty vec + false |
| Profile engine swap fails | Log error; keep existing engine; emit `active-game-changed` with warning |

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src-tauri/src/game_detection.rs` | Create | GameDetector thread, poll loop, Win32 calls, event emission |
| `src-tauri/src/lib.rs` | Modify | Add `mod game_detection`; init `SettingsState.saved_defaults`, `ActiveGameState`, `GameDetectorState`; add `game-changed` listener for auto-switch; add `active-game-changed` emission on settings save |
| `src-tauri/src/commands.rs` | Modify | Add `GameProfile` struct; extend `Settings` with `profiles` + `show_debug`; add 6 new commands; modify `save_settings` to update `saved_defaults` |
| `src-tauri/src/settings.rs` | Modify | No function changes needed (serde handles defaults) |
| `src-tauri/Cargo.toml` | Modify | Add `Win32_System_ProcessStatus` and `Win32_UI_Shell` features to `windows` crate |
| `src/result/index.html` | Modify | Add `<div id="debug-line">` inside `#app-wrapper` |
| `src/result/result.js` | Modify | Add debug-line listener for `active-game-changed` and `settings-changed` |
| `src/write/index.html` | Modify | Add `<div id="debug-line">` inside `#app-wrapper` |
| `src/write/write.js` | Modify | Add debug-line listener for `active-game-changed` and `settings-changed` |
| `src/settings/index.html` | Modify | Add "Game Profiles" section with CRUD UI + debug toggle + auto-fill button |
| `src/settings/settings.js` | Modify | Add profile CRUD functions, auto-fill from `get_active_game`, debug toggle, form rendering |

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `GameProfile` matching (case-insensitive, multi-process) | Rust `#[test]` with mock profiles |
| Unit | Settings serde backward compatibility | Load JSON without `profiles`/`show_debug`, verify defaults |
| Unit | Profile override application | Apply `Some` fields, verify `None` fields unchanged |
| Integration | GameDetector emit + handler | Mock `game-changed` event, verify settings override |
| Integration | Full round-trip: detect → match → override → revert | Manual test switching foreground apps |
| E2E | Settings UI CRUD | Manual: add/edit/delete profiles, verify persistence |
| E2E | Debug indicator visibility | Manual: toggle show_debug, verify overlay updates |

## Migration / Rollout

No migration required. `serde(default)` ensures `profiles: []` and `show_debug: false` on existing `settings.json` files. Rollback: remove `game_detection.rs` and its init; revert `tauri.conf.json`; old settings load fine.

## Resolved Questions

- **Profile ID**: Use `display_name` as the profile identifier. Simple string, no UUID or slug. Each profile's `id` field equals its `display_name`.
- **Settings banner**: Yes — when a game profile is active, Settings shows a banner: "Active: Path of Exile" with the overridden values. The translation overlay debug line also shows the profile name.
- **Poll interval**: Fixed at 1000ms. Not configurable. Imperceptible delay and minimal CPU usage.