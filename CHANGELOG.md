# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.9.2] - 2026-06-10

### Changed
- **Color palette adjusted**: backgrounds changed from blue-tinted grays (`#1a1a2e`, `#16213e`) to pure grays (`#1f2937`, `#111827`). Section titles are now gray-white instead of blue. Blue accent (`#4e9af1`) is reserved only for focus rings and links. Terminal green continues for terminal cues.
- **Custom title bar**: the main Settings window now has a custom terminal-style title bar replacing native Windows decorations. Includes `[ — ]` minimize (to taskbar) and `[ X ]` close (hide, not exit) buttons. The title bar is draggable — same pattern as the result window. Window is no longer resizable.
- **Custom terminal-style selects**: source language, target language, primary engine, and overlay position dropdowns now use a custom component with `>` arrow instead of native browser chrome. The native `<select>` is preserved in the DOM for form values.
- **Checkbox visualization corrected**: checkboxes now properly show `[ ]` when unchecked and `[x]` when checked via CSS `::before` pseudo-elements. The hardcoded span text is no longer used.
- **API key help button relocated**: moved from next to the Primary Engine dropdown to a `[ HOW TO GET API KEYS ]` button above the engines list, where it's contextually relevant.
- **Engines list footer**: version updated to v0.9.2.

### Fixed
- **"Error checking key" no longer shows for disabled engines**: engine status messages (API key stored, error checking key, etc.) are only displayed when the engine's checkbox is checked. Disabled engines show no status text at all. Re-enabling an engine re-checks its key storage.
- **Test All Keys respects checkbox state**: per-engine status updates during testing are guarded by checkbox state, preventing stale status from appearing on engines unchecked mid-test.

### Notes
- Pure UI/UX refinements. No backend changes, no new Tauri commands, no data migration. All settings, API keys, profiles, and history persist unchanged from v0.9.1.
- The main Settings window now has `decorations: false` and `resizable: false` in tauri.conf.json. The existing `on_window_event` handler in `lib.rs` is preserved as a safety net.
- Custom select component is vanilla JS (no framework). Can be extended with keyboard navigation in a future change.
- Profile form selects (`#profile-source-lang`, `#profile-target-lang`, `#profile-engine`) still use native browser chrome — out of scope for v0.9.2.

## [0.9.1] - 2026-06-10

### Added
- Groq translation engine (model: `llama-3.1-8b-instant`). OpenAI-compatible API, free tier with generous rate limits (6K TPM, 500K TPD). Add your Groq API key in Settings > Translation Engines to enable. Groq appears as a new paid engine alongside Gemini, DeepL, and DeepSeek — opt-in only, not enabled by default.
- API key help modal now includes Groq setup instructions.

### Notes
- Additive change. No existing engines removed, no defaults changed, no settings migration required. Users on v0.9.0 keep all their current settings; Groq is just an additional option in the engine list.
- DeepSeek remains fully supported. Groq is added as an alternative for users who want a free tier or faster inference.

## [0.9.0] - 2026-06-10

### Changed
- **Full Settings panel visual redesign**: hybrid aesthetic combining console-app feel (gray dark theme, blue accents) with terminal aesthetic (monospace body text, green accents for terminal cues, custom `[x] [ ]` checkboxes, `>` prompt prefix on user-input fields).
- **Engines + API Keys consolidated** into a single section. Each engine's checkbox and API key input are stacked. A single `[ TEST ALL KEYS ]` button at the bottom tests all enabled engines' keys at once and automatically saves successful ones.
- **Logs panel** converted from an inline expandable panel to a full-screen modal with color-coded log lines (red for errors, yellow for warnings, green for success, gray for default).
- **Game Profiles** section redesigned with the new aesthetic (monospace inputs, terminal-style action buttons).
- **History** section redesigned: each entry rendered as a single terminal-style line.
- **API Key Help modal** redesigned with green border and monospace content.

### Notes
- Primarily a UI change (CSS, HTML, JS). The only backend addition is a new `clear_logs` Tauri command (5 lines in `commands.rs` + 1-line registration in `lib.rs`) to support the Clear button in the new logs modal. The in-memory log buffer previously had no clear path.
- No changes to settings data model or storage. All existing IDs and event signatures are preserved — save logic and event listeners work unchanged.
- The app version in the Settings footer is now `v0.9.0`.

## [0.8.6] - 2026-06-10

### Fixed
- Freeze overlay now hides immediately after OCR detects text, before the translation roundtrip. The user returns to the game the moment text is detected, instead of waiting 2-5 seconds for the translation model to respond. The result overlay appears separately when the translation completes.

### Added
- App version displayed in Settings footer (bottom-right). Lets the user verify at a glance which version is running.

## [0.8.5] - 2026-06-09

### Fixed
- Game Profile UI not rendering saved profiles on app restart. The `list_profiles` and `get_active_game` Tauri commands now use the existing `invokeWithRetry` helper (same pattern as `get_settings`) to handle transient "state not managed" errors on startup. Also: `closeProfileForm()` is now always called in a `finally` block in `saveProfile`, so the form closes even if the post-save re-fetch fails. Defensive `closeProfileForm()` call added to DOMContentLoaded to guarantee correct initial state.

## [0.8.4] - 2026-06-09

### Changed
- API keys now stored in plain JSON file (%APPDATA%/overlex/api_keys.json) instead of Windows Credential Manager. Resolves silent fallback to Google Translate when process elevation changes between sessions.

### Removed
- `keyring` crate dependency

### Added
- User-visible warning when paid engines have no API key configured (was previously silent)

### Migration Notes
- After upgrading to v0.8.4, API keys must be re-entered in Settings. The settings panel will show a warning banner on first launch listing which engines need configuration.

## [0.8.3] - 2026-06-09

### Fixed
- CSP now allows API calls to Gemini, DeepL, and DeepSeek (paid engines were silently blocked)
- API keys now explicitly loaded from Windows Credential Manager on startup and during game profile auto-switch (previous implicit fallback silently swallowed errors)
- Game profile overrides now apply immediately at app startup (was waiting for first 1-second polling cycle)

### Added
- `GameProfile.context_prompt` field for per-game lore/terminology sent to AI engines as system context (auto-generated, no UI editor)
- `build_context_prompt()` function with 5 unit tests, propagating context through the Engine trait
- New `GameDetector::detect_current_game()` one-shot detection method for startup hydration

## [0.3.0] - 2026-06-06

### Fixed
- API keys now persist to Windows Credential Manager on save (were lost on restart)
- Settings now returns saved defaults instead of profile-overridden values
- Added `get_active_settings` command for overlays that need effective settings

## [0.2.0] - 2026-06-04

### Added
- Game detection with automatic profile switching
- Gemini 2.0 Flash + DeepL translation with adaptive fallback chain
- Per-engine API key management with status indicators
- Overlay shows which translation engine is active

## [0.1.0] - 2026-04-17

### Added
- Initial Tauri 2 project scaffold
- System tray icon with show/hide toggle
- Basic settings UI
- Google Translate as baseline engine