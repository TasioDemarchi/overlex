# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.9.9] - 2026-06-12
- fix: OCR and Write hotkey toggles now work on unlimited presses (was: thread died after first toggle because `return;` in the match arm exited the entire closure, killing the message pump)

## [0.9.8] - 2026-06-12
- fix: Write and OCR hotkey toggles now work consistently across multiple presses by replacing IsWindowVisible with manual state tracking

## [0.9.7] - 2026-06-12
- feat: Write hotkey (Ctrl+Shift+W by default) now toggles the write window — press it again to close the open write overlay

## [0.9.6] - 2026-06-11
- feat: OCR hotkey (Ctrl+Shift+T) now toggles the result window — press it again to close the open translation overlay

## [0.9.5] - 2026-06-11

- fix: attempt to fix Esc key leaking to game by toggling WS_EX_NOACTIVATE on result window (did not resolve the issue — Esc still reaches the game; superseded by v0.9.6 with a keyboard hook approach)

## [0.9.4] - 2026-06-10

### Changed
- **Removed `[ ]` from window control buttons**: minimize button now shows `—`, close button shows `X` without surrounding brackets. Buttons remain functional (minimize to taskbar, hide window).
- **Fixed custom select wrappers loading stale values**: refactored `DOMContentLoaded` initialization order so native select values are set BEFORE custom terminal-select wrappers are created. Fixes the bug where source-lang, target-lang, overlay-position, and primary-engine dropdowns displayed the HTML default instead of the saved value after restart.
- **Settings footer**: version updated to v0.9.4.

### Fixed
- **Overlay position dropdown displays saved value on restart**: the root cause was that `createTerminalSelect` was called before `invoke('get_settings')` resolved, capturing the initial HTML option ("Near Selection") instead of the loaded value ("Top Left" or whatever the user saved). Fixed by reordering: settings load → `setNativeSelectValues` → `createTerminalSelect`.
- **Same fix applies to source-lang, target-lang, and primary-engine selects**: all 4 custom selects now correctly reflect saved values on app restart.

### Notes
- Bug fix + minor UI refinement. Zero backend changes, zero new Tauri commands, no data migration. All settings, API keys, profiles, and history persist unchanged from v0.9.3.
- Added `setNativeSelectValues(settings)` helper function to centralize native select value-setting logic.
- Added defensive `createTerminalSelect` refresh after save to ensure wrappers always match saved values.

## [0.9.3] - 2026-06-10

### Changed
- **Removed `api-key-missing-banner`**: the global warning banner that appeared at the top of the Settings panel when paid engines were enabled without API keys is removed. The per-engine inline status (already present under each engine's API key input) is the sole feedback mechanism for missing keys. The backend still emits the `api-key-missing` event — it's just no longer consumed by the frontend.
- **Fixed static checkbox double-render**: the 8 static HTML checkboxes no longer have hardcoded `[ ]` text in their `<span class="cb-display">` elements. The CSS `::before` pseudo-element is now the single source of truth for bracket rendering, eliminating the `[x][ ]` visual bug.
- **Removed `[ ]` brackets from action buttons**: all action buttons now have plain text labels (e.g., `SAVE SETTINGS`, `TEST ALL KEYS`, `EDIT`, `CLOSE`) instead of `[ SAVE SETTINGS ]`. Brackets are preserved only on window controls (`[ — ]`, `[ X ]`) and checkboxes (`[ ]`, `[x]`), where they have semantic meaning.
- **Styled scrollbar**: added WebKit scrollbar CSS matching the terminal aesthetic (8px wide, gray `--border-strong` color, `--terminal-radius` rounding, darker on hover, green-tinted on active).
- **Removed `<h1>OverLex Settings</h1>`**: the window title bar (added in v0.9.2) already shows the app name, making the h1 redundant.
- **Settings footer**: version updated to v0.9.3.

### Notes
- Pure UI/UX refinements. Zero backend changes, zero new Tauri commands, no data migration. All settings, API keys, profiles, and history persist unchanged from v0.9.2.
- The `api-key-missing` backend event is still emitted by `lib.rs` but no longer consumed by the frontend. If a future UI wants the banner back, the event is still available.
- The `api-key-modal-close` button text changed from `[ X ]` to `CLOSE`. The `logs-modal-close` button changed from `[ CLOSE ]` to `CLOSE`.

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