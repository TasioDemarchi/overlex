# Delta Spec: game-detection-profiles

## ADDED Requirements

### REQ-01: Game Process Detection
The system MUST poll the foreground window process name every ~1s and emit `game-changed` on change. The thread MUST start on launch and stop cleanly on exit.

- **Scenario: Process switch**
  - GIVEN the app is running
  - WHEN the user switches to `poe2.exe`
  - THEN `game-changed` emits with `process_name: "poe2.exe"`

- **Scenario: No foreground**
  - GIVEN the lock screen is active
  - WHEN the poll fires
  - THEN `game-changed` emits with `process_name: null`

- **Scenario: Same process, different window**
  - GIVEN two windows share one process
  - WHEN focus moves between them
  - THEN no `game-changed` event fires

### REQ-02: Exclusive Fullscreen Detection
The system MUST detect exclusive fullscreen via Win32 styles or `SHQueryUserNotificationState`, and include `fullscreen_exclusive: bool` in the `game-changed` payload.

- **Scenario: Exclusive fullscreen**
  - GIVEN a game in exclusive fullscreen
  - WHEN the poll fires
  - THEN the payload contains `fullscreen_exclusive: true`

- **Scenario: Windowed mode**
  - GIVEN a game in borderless windowed
  - WHEN the poll fires
  - THEN the payload contains `fullscreen_exclusive: false`

### REQ-03: Game Profiles
The system MUST support `GameProfile` with `process_names`, `display_name`, `source_lang`, `target_lang`, `engine`, `ocr_preprocessing`, and `ocr_binarize`. Matching MUST be case-insensitive.

- **Scenario: Match applies**
  - GIVEN a profile for `["poe2.exe"]`
  - WHEN the foreground becomes `poe2.exe`
  - THEN the profile becomes active

- **Scenario: No match defaults**
  - GIVEN no profile for `notepad.exe`
  - WHEN the foreground becomes `notepad.exe`
  - THEN default settings stay active

### REQ-04: Auto-Switch Settings on Game Detection
The system MUST apply matching profile overrides instantly and revert to saved defaults on non-match.

- **Scenario: Switch in**
  - GIVEN a profile sets `target_lang: "ja"` for `poe2.exe`
  - WHEN `game-changed` reports `poe2.exe`
  - THEN active `target_lang` becomes `"ja"` instantly

- **Scenario: Switch out**
  - GIVEN `poe2.exe` was active with overrides
  - WHEN `game-changed` reports a non-match
  - THEN settings revert to saved defaults instantly

### REQ-05: Debug Indicator on Overlays
The system MUST show a semi-transparent monospace debug line at the bottom of `result` and `write` overlays when `show_debug` is true, completely hidden when false.

- **Scenario: Game detected**
  - GIVEN `show_debug: true` and `poe2.exe` is active with Gemini
  - WHEN an overlay appears
  - THEN the line shows `poe2.exe · Gemini API`

- **Scenario: Fullscreen warning**
  - GIVEN `show_debug: true` and exclusive fullscreen detected
  - WHEN an overlay appears
  - THEN the line shows `poe2.exe ⚠ Fullscreen · Gemini API`

- **Scenario: Hidden**
  - GIVEN `show_debug: false`
  - WHEN an overlay appears
  - THEN no line is rendered and no space reserved

### REQ-06: Settings UI for Game Profiles
The Settings UI MUST support add, edit, delete, and list for profiles, plus a `show_debug` toggle.

- **Scenario: Add**
  - GIVEN the user opens Game Profiles
  - WHEN they save a new profile
  - THEN it persists and the list updates

- **Scenario: Delete**
  - GIVEN a profile "POE2" exists
  - WHEN the user clicks delete
  - THEN it is removed and the list updates

### REQ-07: Tauri Commands for Profiles
The system MUST expose `add_profile`, `remove_profile`, `update_profile`, `list_profiles`, `get_active_game`, and `toggle_debug`.

- **Scenario: List**
  - GIVEN two profiles exist
  - WHEN `list_profiles` is invoked
  - THEN it returns a `Vec<GameProfile>` with two items

- **Scenario: Active game**
  - GIVEN `poe2.exe` is foreground and matches a profile
  - WHEN `get_active_game` is invoked
  - THEN it returns `{process: "poe2.exe", profile: "POE2"}`

## MODIFIED Requirements

### Settings: Backward-compatible extension
(Previously: Settings had no `profiles` or `show_debug`.)

The Settings struct MUST include `profiles: Vec<GameProfile>` and `show_debug: bool` with `serde(default)`.

- **Scenario: Migration**
  - GIVEN an old `settings.json`
  - WHEN the app loads settings
  - THEN `profiles` defaults to empty and `show_debug` to `false`

- **Scenario: Round-trip**
  - GIVEN settings with profiles and `show_debug: true`
  - WHEN saved and reloaded
  - THEN all fields restore correctly
