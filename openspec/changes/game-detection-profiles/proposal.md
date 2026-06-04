# Proposal: game-detection-profiles

## Intent

Auto-switch translation settings when a game becomes the foreground window. Poll the foreground process name every second and match it against user-configured game profiles to apply language, engine, and OCR overrides instantly.

## Scope

### In Scope
- Background thread polling foreground window process name every 1s
- `game-changed` Tauri event emission on process change
- Game profiles with process name matching, language/engine/OCR overrides
- Settings CRUD commands for profiles
- Debug indicator overlay window (toggleable, draggable)

### Out of Scope
- Translation engine adapters (Phase 2)
- Personal glossary / search overlay (Phase 3)
- Smart lookup / auto-enrich (Phase 4)
- Adaptive OCR preprocessing (Phase 5)
- Continuous capture buffer, AI vision OCR, multi-profile priority rules

## Capabilities

### New Capabilities
- `game-detection`: Win32 foreground process polling and event emission
- `game-profiles`: JSON-stored profile overrides with CRUD commands
- `debug-indicator`: Tiny always-on-top webview showing active process + engine

### Modified Capabilities
- `settings`: Add `profiles: Vec<GameProfile>` and `show_debug: bool` with serde default

## Approach

Spawn `GameDetector` thread using `GetForegroundWindow` → `GetWindowThreadProcessId` → `OpenProcess` → `GetModuleFileNameExW` (needs `Win32_System_ProcessStatus`). On change, emit `game-changed`. `lib.rs` listens and swaps active settings via the existing engine abstraction. Profile CRUD in `commands.rs`. Debug window in `tauri.conf.json` + `src/debug/`.

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Anti-cheat false positive | Low | Only read process name; no hooks or injection |
| Settings migration failure | Low | `serde(default)` yields empty profiles + false debug |
| Performance regression | Low | 1s poll; debug window only when enabled (~5MB) |
| Multiple game executables | Low | `process_names: Vec<String>` supports variants |

## Files Affected

| File | Change |
|---|---|
| `src-tauri/src/game_detection.rs` | New: GameDetector + thread |
| `src-tauri/src/commands.rs` | Add profile CRUD commands |
| `src-tauri/src/settings.rs` | Add `GameProfile`, `profiles`, `show_debug` |
| `src-tauri/src/lib.rs` | Init detector, handle `game-changed`, swap engine |
| `src-tauri/Cargo.toml` | Add `Win32_System_ProcessStatus` |
| `tauri.conf.json` | Add `debug` window |
| `src/settings/*` | Add profiles UI + debug toggle |
| `src/debug/*` | New: debug overlay |

## Dependencies

- Translation engine abstraction in `lib.rs`
- Hotkey thread pattern as reference
- Win32 `Win32_System_ProcessStatus` feature

## Rollback Plan

Remove `game_detection.rs` and its init from `lib.rs`. Revert `tauri.conf.json`. Old `settings.json` loads fine due to `serde(default)`.

## Success Criteria

- [ ] Foreground process name updates in debug window within 1s of switching apps
- [ ] Matching profile auto-switches source/target language and engine
- [ ] Non-matching foreground reverts to default settings
- [ ] Profile CRUD works from Settings UI and persists to `settings.json`
- [ ] Debug overlay toggles on/off and is draggable
