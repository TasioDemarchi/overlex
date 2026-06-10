# Architecture Decisions

This file documents key architectural decisions for OverLex. Each ADR is numbered and includes context, the decision, and its consequences. Decisions are documented retroactively based on codebase analysis.

---

## ADR-001 — Windows-only target

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: OverLex needs low-level Windows APIs for screen capture (DXGI/GDI), OCR (Windows.Media.Ocr), global hotkeys (RegisterHotKey), and window management (WS_EX_NOACTIVATE, acrylic blur). These are Windows-only Win32 APIs. Supporting other platforms would require completely different implementations for every system-level feature.
- **Decision**: Target Windows 10/11 exclusively. All system-level code uses Win32 APIs via the `windows` crate. Conditional compilation (`#[cfg(windows)]`) is used sparingly since the entire app is Windows-only.
- **Consequences**:
  - No macOS/Linux support. Not a future consideration.
  - Full access to Windows-specific features (Credential Manager, acrylic blur, native OCR).
  - Installer is NSIS-only (no DMG/AppImage).
  - Testing requires Windows — no CI tests possible on non-Windows runners.

---

## ADR-002 — Vanilla JS frontend (no framework)

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: OverLex's frontend requirements are minimal: a settings form, a fullscreen canvas, a text input overlay, and a result display. The user has 0 dev experience and a framework (React, Vue, etc.) would add complexity to the build pipeline and learning curve.
- **Decision**: Use plain HTML/CSS/JS for all 4 webview windows. No bundler, no framework, no build step for frontend. Tauri serves the HTML files directly.
- **Consequences**:
  - Zero frontend build time. Fast iteration.
  - No npm dependency hell for frontend packages.
  - No component system, no reactivity — state is managed imperatively.
  - Harder to scale if UI complexity grows significantly.

---

## ADR-003 — 4 separate webview windows (not a SPA)

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Different overlay modes have fundamentally different window requirements: the freeze overlay must be fullscreen and non-transparent, the result overlay must be transparent and always-on-top but never steal focus, and the write overlay needs input capture. Consolidating into a single SPA would require complex routing and window management.
- **Decision**: Maintain 4 separate Tauri webview windows with distinct configurations:
  - `main`: Settings panel (normal window, hidden by default)
  - `freeze`: Fullscreen screenshot overlay (no decorations, always-on-top)
  - `result`: Translation result (transparent, always-on-top, WS_EX_NOACTIVATE, skip taskbar)
  - `write`: Write mode input (transparent, always-on-top, skip taskbar)
- **Consequences**:
  - Each window has its own HTML/CSS/JS — some code duplication between windows.
  - Communication is via Tauri events (emit/listen pattern).
  - Window-specific behaviour (acrylic blur, no-focus) is set per-window config.
  - CONFIRMED by user: this is the correct architecture, do not consolidate.

---

## ADR-004 — DXGI Desktop Duplication with GDI fallback

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Screen capture is needed for OCR mode. DXGI Desktop Duplication is fast (~50ms) but can fail in certain scenarios (RDP session,某些 GPU configurations). GDI BitBlt is slower (~6s) but universally available on Windows.
- **Decision**: Try DXGI first, fall back to GDI BitBlt if DXGI fails. Both implementations use raw RGBA pixel output, encoded to PNG asynchronously.
- **Consequences**:
  - Fast capture in most cases (~50ms DXGI).
  - Graceful fallback for edge cases (~6s GDI). User sees "Translating..." overlay while waiting.
  - Two code paths to maintain.
  - PNG encoding runs on a background thread to avoid blocking the freeze overlay display.

---

## ADR-005 — Windows.Media.Ocr for OCR

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: OCR is required to extract text from screenshots. Options included Tesseract (C++ dependency, large binary, slow), Windows.Media.Ocr (built-in, zero install size, fast), and cloud OCR (requires internet, latency).
- **Decision**: Use Windows.Media.Ocr (WinRT API via `windows` crate). Requires target language pack installed in Windows Settings.
- **Consequences**:
  - Zero additional binary size — OCR is built into Windows 10/11.
  - Fast performance on modern hardware.
  - User must install the target language OCR pack in Windows (instructions in docs).
  - Quality depends on Windows OCR engine — handles common game fonts reasonably well but may struggle with stylized fonts.
  - Smart line-joining heuristic for CJK and game dialogue text.

---

## ADR-006 — Multi-engine translation with adaptive fallback chain

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Single-engine translation is fragile — the engine may be down, rate-limited, or produce poor quality. Different engines excel at different language pairs and text types. MyMemory is unreliable for game terminology.
- **Decision**: Implement a `TranslationChain` that wraps multiple engines. Fallback order: primary engine → other enabled paid engines (in user-configured order) → Google GTX (last resort). MyMemory is excluded from the fallback chain (only used as primary).
- **Consequences**:
  - Resilient translation: if the primary engine fails, the chain tries other engines automatically.
  - Google GTX is always the last resort fallback (free, no API key, always available).
  - User can configure which engines are enabled and their order.
  - Each engine in the chain has a 15s timeout.

---

## ADR-007 — Settings two-tiers: saved_defaults + active with profile overrides

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Game profiles need to override certain settings (language, engine, OCR options) when a specific game is detected. Previously, profile overrides were applied directly to the saved settings, causing contamination: switching between games would leave stale overrides in the persisted data.
- **Decision**: Maintain two separate Settings instances:
  1. `saved_defaults` — The persisted baseline (never modified by profile overrides).
  2. `settings` (active) — The effective runtime settings, built by cloning `saved_defaults` and applying profile overrides on top.
   Profile overrides only affect the active settings, never touch `saved_defaults`.
- **Consequences**:
  - Profiles no longer contaminate saved defaults.
  - Switching games (or back to no-game) correctly resets to defaults.
  - Switching back to a game re-applies the profile overrides.
  - Slightly more memory (two Settings instances kept in state).
  - CONFIRMED BY USER: This is the correct architecture. Profiles must NOT modify defaults.

---

## ADR-008 — SQLite with FTS5 for translation history

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Translation history needs to be searchable, persistent, and efficient. Options included JSON file (simple but no search), SQLite (embedded, zero config), and a full DBMS (overkill).
- **Decision**: Use SQLite via `rusqlite` with `bundled` feature (no external SQLite dependency). Use FTS5 virtual table for full-text search on original and translated text. Database stored at `%APPDATA%/overlex/history.db`.
- **Consequences**:
  - Zero external dependencies — SQLite is compiled into the binary.
  - Fast full-text search via FTS5 (BM25 ranking).
  - Auto-sync via triggers (FTS index stays in sync with translations table).
  - ~2.5MB binary size increase from bundled SQLite.
  - History can be exported to JSON or CSV.

---

## ADR-009 — API keys in Windows Credential Manager (DPAPI)

- **Date**: 2026-06-09 (retroactive)
- **Status**: Superseded (2026-06-09 by ADR-016)
- **Context**: API keys for paid translation engines (Gemini, DeepL, DeepSeek) are sensitive credentials. Storing them in settings.json (plaintext on disk) is a security risk. Options included environment variables (session-only), encrypted config file (need key management), and OS credential manager.
- **Decision**: Use `keyring` crate to store API keys in Windows Credential Manager, which encrypts them with DPAPI (user-bound, machine-bound). Keys are never written to settings.json.
- **Consequences**:
  - API keys are encrypted at rest by Windows DPAPI.
  - Keys survive app uninstall/reinstall (Credential Manager is separate).
  - Keys are bound to the Windows user account — no other user can access them.
  - Keys must be fetched on every engine creation (slight latency but cached in memory).
  - No cross-platform credential store (Windows-only is fine per D1).
- **Superseded note**: The `keyring` crate wraps Windows Credential Manager via COM, which fails silently when the process elevation context changes between sessions (e.g., admin install → normal launch). There is no reliable recovery path without re-prompting the user. The failure mode is silent because `get_api_key()` returns `Err` and the translation chain falls back to google_gtx without any user-visible error. See ADR-016 for the replacement.

---

## ADR-010 — Game detection with 1-second polling

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: OverLex needs to detect which game (if any) the user is currently playing to auto-apply game profile overrides. Options included event-driven (SetWinEventHook — complex, may miss events), polling (simple, reliable), and HID monitoring (overkill).
- **Decision**: Run a background OS thread that calls `GetForegroundWindow()` every 1000ms, extracts the process name, and emits a `game-changed` event. On match with a game profile, the auto-switch handler applies profile overrides and rebuilds the engine chain.
- **Consequences**:
  - 1-second delay in detecting game switches (acceptable for this use case).
  - Minimal CPU overhead (one Win32 API call per second).
  - Works reliably regardless of how the foreground window changes.
  - Fullscreen exclusive mode detection via `GetMonitorInfoW` comparison.
  - Shutdown signal via AtomicBool on app exit.

---

## ADR-011 — Overlays with acrylic blur + WS_EX_NOACTIVATE

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Translation overlays must be visually unobtrusive (transparent with blur) and must never steal focus from the game. Without WS_EX_NOACTIVATE, clicking on or interacting with the overlay window could cause the game to lose focus.
- **Decision**: Apply acrylic blur effect (via `window-vibrancy` crate) to result and write windows. Set `WS_EX_NOACTIVATE` (0x08000000) extended window style on the result window to prevent focus stealing. All overlay windows use `skipTaskbar: true` and `alwaysOnTop: true`.
- **Consequences**:
  - Overlays are visually clean with acrylic blur background.
  - Game never loses focus when overlays appear or are clicked.
  - Result window is the only one with WS_EX_NOACTIVATE (needed for interaction-less display).
  - Write window intentionally captures focus (user is typing) — focus is restored to the game foreground window on dismiss.

---

## ADR-012 — In-memory log buffer

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: Debugging a shipped Windows-only app without a debugger requires some form of logging. Options included file logging (disk I/O, privacy concerns), Windows Event Log (complex), cloud logging (privacy, requires internet), and in-memory buffer (simple, zero privacy risk).
- **Decision**: Maintain a global `Mutex<Vec<LogEntry>>` buffer with 200-entry cap. Both Rust backend and JS frontend can write to it. Logs are exposed via the `get_recent_logs` Tauri command. A "Show debug" checkbox in settings toggles a live log viewer.
- **Consequences**:
  - Zero disk I/O for logs (no wear on SSDs, no privacy concerns).
  - Circular buffer — always the most recent 200 entries.
  - Logs lost on app restart (acceptable for debugging purposes).
  - Users can share logs for troubleshooting via the settings panel.
  - Hook macro `app_log!()` for ergonomic logging from Rust.

---

## ADR-013 — Content Security Policy (CSP)

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted (Resolved 2026-06-09)
- **Context**: CSP is needed to restrict what network requests the webview can make, preventing XSS and unauthorized data exfiltration. However, the translation engines need to make API calls to their respective endpoints.
- **Decision**: Set CSP in `tauri.conf.json` that allows `connect-src` to `*.googleapis.com` and `api.mymemory.translated.net`. This covers Google GTX and MyMemory.
- **Consequences**:
  - RESOLVED in v0.8.3: Gemini, DeepL, and DeepSeek endpoints added to CSP `connect-src` allowlist. All 5 supported engines now work out of the box.
  - Approach: hardcoded allowlist (KISS — only 5 engines). Dynamic generation deferred to a future change if engine count grows significantly.

---

## ADR-014 — Custom hotkey capture via Win32 RegisterHotKey

- **Date**: 2026-06-09 (retroactive)
- **Status**: Accepted
- **Context**: OverLex needs global hotkeys that work even when the game is in focus (background). Options included Tauri's global shortcut plugin (limited customization), Win32 RegisterHotKey (full control, no extra dependency), and low-level keyboard hook (heavy, potential anti-cheat flags).
- **Decision**: Use Win32 `RegisterHotKey` API with a dedicated message pump thread. Hotkey strings are parsed from settings format (e.g., `CTRL+SHIFT+T`) into MOD_* flags + virtual key codes. Three hotkeys: OCR capture, write mode, and language swap.
- **Consequences**:
  - Hotkeys work globally, even when game is in focus.
  - No extra plugin dependency — uses raw Win32 API.
  - Dedicated thread with message pump ensures reliable WM_HOTKEY delivery.
  - Hotkey registration is re-done when settings change (unregister old, register new).
  - Limited to hotkeys that Win32 RegisterHotKey supports (modifier + non-modifier key combos).

---

## ADR-015 — Auto-generated `context_prompt` for game profiles

- **Date**: 2026-06-09
- **Status**: Accepted
- **Context**: The PRD section 4.6 specifies that game profiles can include a `context_prompt` — a free-text description of the game's lore, characters, terminology, and translation preferences — that gets sent to AI engines (Gemini, DeepL, DeepSeek) as system context. The user confirmed (collaborative session 2026-06-09) that the prompt should be **auto-generated by the system** from current app state, not manually edited in a UI form. The `GameProfile.context_prompt` field is the user-editable lore/terminology per profile; the system wraps it into a final system prompt using a template function.
- **Decision**:
  1. Add `context_prompt: Option<String>` to `GameProfile` struct with `#[serde(default)]` for backwards compatibility.
  2. Add `build_context_prompt()` function that takes `(game_name, profile_prompt, source_lang, target_lang)` and returns `Option<String>` — returns `None` when there's no relevant context.
  3. Extend the `TranslationEngine` trait with `context_prompt: Option<&str>` as the 5th positional parameter on the `translate` method. Engines that support it (Gemini, DeepSeek) APPEND it to the existing system instruction. DeepL uses it as the API `context` field (replacing the process-name context since DeepL's field is bounded and `context_prompt` is richer). Engines that don't (Google GTX, MyMemory) ignore it via `_context_prompt` prefix.
  4. The 3 translation entry points in `commands.rs` (`translate_text`, `ocr_capture_region`, `translate_chat`) all construct the context_prompt from the active game + matched profile and pass it to the chain.
  5. No UI editor in this change. Future change may add a read-only display in settings.js showing the resolved prompt for the active profile.
- **Consequences**:
  - AI engines (Gemini, DeepL, DeepSeek) receive game-specific context automatically, improving translation quality for game-specific terms.
  - Google GTX and MyMemory continue to work unchanged (parameter is ignored).
  - The Engine trait change is a breaking signature change, but all 7 implementations were updated atomically — no compatibility shim needed since the project has no external consumers.
  - 5 unit tests for `build_context_prompt` cover all branches (no input, game name only, profile prompt only, both, period deduplication).
  - Engines that supported per-engine context (Gemini/DeepSeek had system instructions, DeepL had a `context` field) now have a richer and more accurate source of context.

---

## ADR-016 — API keys in plain JSON file (replaces Windows Credential Manager)

- **Date**: 2026-06-09
- **Status**: Accepted
- **Context**: The `keyring` crate (ADR-009) wraps Windows Credential Manager via COM. On some Windows configurations, especially when process elevation context changes between sessions (e.g., install as admin, launch as normal user), keyring fails silently — `get_api_key()` returns `Err` and the translation chain falls back to google_gtx with no user-visible error. For a personal single-user desktop app, a plain JSON file in `%APPDATA%` is simpler, debuggable (open in Notepad), and eliminates the entire class of COM-related failures. The user accepts a one-time re-entry of keys after the v0.8.4 upgrade.
- **Decision**: Store API keys in `%APPDATA%/overlex/api_keys.json` with schema `{"version": 1, "keys": {"deepseek": "sk-...", "gemini": "AIza...", "deepl": "fx-..."}}`. File is created with empty `keys` on first read. Writes use atomic rename (`.tmp` → final path) to prevent corruption on crash. Corrupt files are renamed to `.bak` and recreated fresh (same pattern as `settings.json` corruption handling). No automatic migration from Credential Manager — the `api-key-missing` Tauri event notifies the user to re-enter keys in Settings.
- **Consequences**:
  - API keys are plaintext on disk (acceptable for single-user personal PC).
  - Keys persist across NSIS upgrades (installer preserves `%APPDATA%` by convention).
  - Debuggable: user can open `api_keys.json` in Notepad to verify keys exist.
  - No COM dependency — zero silent failure modes from Windows Credential Manager.
  - No encryption at rest. User can delete the file to remove all keys.
  - On upgrade from v0.8.3 to v0.8.4: one-time re-entry of keys required. The `api-key-missing` event makes this discoverable on first startup.

---

## ADR-017 — Decoupled freeze hide from translation completion

- **Date**: 2026-06-10
- **Status**: Accepted
- **Context**: User reported 2-5s perceived delay between region selection and return to game. Root cause: `ocr_capture_region` hides the freeze window at the END of the function, AFTER the translation completes. The translation roundtrip to Gemini/DeepSeek takes 2-5 seconds, during which the user stares at the freeze overlay (the selection rectangle). The result window was already decoupled (event-driven via `translation-result` events), but the freeze hide was not.
- **Decision**: Hide the freeze window immediately after OCR succeeds, before the translation call. The result window is already event-driven via `emit_result` / `translation-result` events, so decoupling the freeze hide is safe — the user returns to the game in ~100ms (OCR time only), and the translation result appears later when the network call completes.
- **Consequences**:
  - User returns to game in ~100ms (OCR time) instead of 2-5s (OCR + translation time).
  - Result window appears later when translation completes (unchanged behavior).
  - Error paths unchanged — all error branches already hide the freeze independently.
  - Existing freeze hide at end of function is kept as a safety net (no-op in normal flow).
- **Alternatives considered**:
  - A2: Full async refactor with `request_id` — rejected as premature abstraction for v0.8.6. The current fire-and-forget approach is simpler and sufficient for the single-window model.

---

## ADR-018 — Settings panel visual redesign — hybrid console + terminal aesthetic

- **Date**: 2026-06-10
- **Status**: Accepted
- **Context**: The Settings panel used a generic dark theme with blue accents and native HTML form controls. While functional, it felt generic and didn't match the "tool for power users" nature of OverLex. The UI needed a distinctive identity that signals "this is a developer-grade tool, not a casual app."
- **Decision**: Adopt a hybrid aesthetic combining:
  1. **Console-app baseline** (Image 1 reference): gray dark theme (`#1a1a2e`, `#16213e`, `#0f0f1a`), sans-serif headings, two-column language distribution, generous spacing.
  2. **Terminal accent** (Image 2 reference): monospace body text (labels, inputs, buttons, cards), ASCII-style checkboxes `[x] [ ]`, prompt-style `>` prefix on user-input fields, green accent (`#51cf66`) for terminal cues.
  - Blue base (`#4e9af1`) for general accents (focus rings, links, primary actions).
  - Green (`#51cf66`) reserved for terminal-specific cues: checkboxes, `>` prefix, section dividers, success states.
  - Custom CSS checkboxes using `:checked + .cb-display` to swap `[ ]` → `[x]` (native `<input type="checkbox">` preserved in DOM for accessibility and form values).
  - Logs converted from inline panel to modal with color-coded log lines.
  - **Exception to the "no backend changes" rule**: The new `[ CLEAR ]` button in the logs modal required a new `clear_logs` Tauri command. The in-memory log buffer (`LOG_BUFFER` in `commands.rs`) previously had no clear path — only `get_recent_logs` (read) and `add_log` (write). The clear feature is 5 lines of Rust (`pub fn clear_logs() { LOG_BUFFER.lock().unwrap().clear(); }` + 1-line registration in `lib.rs`). The frontend was already calling `invoke('clear_logs')` based on the planned feature, but the backend command was missing — this was caught during post-implementation verification and fixed before commit. The plan and CHANGELOG were updated to reflect this.
  - All existing IDs and event signatures preserved — zero new Tauri commands **except** `clear_logs` (see above). All settings save logic and event listeners work unchanged.
- **Consequences**:
  - More opinionated aesthetic — some users may prefer all-sans or all-mono.
  - Terminal-style checkboxes add ~30 lines of CSS but eliminate reliance on browser-native checkbox rendering (which varies across Windows versions).
  - Custom `::before` pseudo-elements for `>` prefix on inputs — requires wrapper divs but keeps HTML IDs unchanged.
  - Engines + API Keys consolidated into one dynamic section, reducing the number of static HTML elements.
  - Logs converted from inline panel to modal — better UX for long log outputs but requires modal open/close JS. Clear button now works via new `clear_logs` Tauri command.
  - The plan was revised after the sub-agent implementation to document the `clear_logs` backend addition. This is the minimum viable backend change to make the logs modal feature complete.
- **Alternatives considered**:
  - A1: All-sans-serif (current approach) — rejected as too generic, doesn't differentiate OverLex from other apps.
  - A2: All-monospace (pure terminal) — rejected as too harsh for headings and would make the panel feel like a raw terminal instead of a structured settings panel.
  - A3: Keep per-engine Test Key buttons — rejected because a single `[ TEST ALL KEYS ]` button is more convenient and reduces visual clutter.
  - A4: Make logs clear client-side only (no backend command) — rejected because the next modal open would re-fetch the old logs, making "clear" useless. Backend clear is the only correct solution.

## Summary

| ADR | Title | Key impact |
|-----|-------|-----------|
| 001 | Windows-only | No cross-platform, full Win32 access |
| 002 | Vanilla JS frontend | Simple but imperative |
| 003 | 4 separate webviews | Correct architecture per user, do not consolidate |
| 004 | DXGI + GDI fallback | Fast capture with graceful degradation |
| 005 | Windows.Media.Ocr | Zero-size OCR, needs language pack |
| 006 | Multi-engine fallback | Resilient translation, Google GTX as last resort |
| 007 | Settings two-tiers | Profiles don't contaminate defaults |
| 008 | SQLite + FTS5 | Searchable history, embedded |
| 009 | Credential Manager | Superseded by ADR-016 — COM failures on elevation change |
| 010 | Game detection polling | Reliable, minimal overhead |
| 011 | Acrylic + WS_EX_NOACTIVATE | Non-intrusive overlays |
| 012 | In-memory log buffer | Zero I/O debugging |
| 013 | CSP | CSP now allows all 5 engines |
| 014 | Custom hotkey capture | Global hotkeys via Win32 |
| 015 | Auto-generated context_prompt | Per-game lore/terminology prompts for AI engines |
| 016 | JSON file API key storage | Plain JSON in %APPDATA%, atomic writes, corrupt recovery |
| 017 | Decoupled freeze hide | User returns to game immediately after OCR, not after translation |
| 018 | Hybrid console + terminal UI | Settings panel redesign with `[x] [ ]` checkboxes, `>` prompts, green terminal accents |
| 019 | docs/changes/ folder for plans | Change plans live in `docs/changes/<change-name>/plan.md`, ADRs stay flat in `decisions.md` |
| 020 | Add Groq as alternative paid engine | New Groq engine with llama-3.1-8b-instant model, OpenAI-compatible adapter, free tier, opt-in via enabled_engines |
| 021 | v0.9.2 UI refinements | Gray palette predominance, custom selects, fixed checkbox viz, status visibility fix, help modal relocated, custom title bar |

---

## ADR-021 — v0.9.2 UI refinements: gray predominance, custom selects, title bar

- **Date**: 2026-06-10
- **Status**: Accepted
- **Context**: After shipping v0.9.0 (hybrid console + terminal aesthetic), user reviewed the redesign and identified 5 issues plus wanted a custom title bar for the main Settings window. The blue-dominant palette from the original tokens (`#1a1a2e`, `#16213e`, `#0f0f1a`) created a blue/violet-heavy look that didn't match the "Console Settings" reference image (which uses pure grays). Checkbox state visualization was broken — always showing `[ ]` regardless of checked state. The `checkEngineKeyStatus()` function was called for all paid engines even when disabled, causing confusing "Error checking key" messages. The API key help modal trigger (`?` button) was next to the Primary Engine dropdown, semantically wrong. Native browser selects didn't match the terminal aesthetic. Additionally, the user wanted custom window controls for the main Settings window.
- **Decision**: 
  1. **Color palette**: Shift from blue-tinted dark to pure gray dark (`#1f2937`, `#111827`, `#0b1220`). Section titles (`h2`) use `var(--text-primary)` (gray-white) instead of blue. `.small-btn:hover` uses `var(--text-secondary)` (gray) instead of blue. Blue accent (`#4e9af1`) kept only for `:focus` rings, `:focus-visible` outlines, and `<a>` links. Terminal green (`#51cf66`) kept for checkboxes, `>` prefixes, section dividers, and success states.
  2. **Checkbox fix**: Use CSS `::before` pseudo-elements to render `[ ]` / `[x]` content. The `.cb-display` span's textContent is removed from JS (CSS overrides). `:checked + .cb-display::before { content: '[x]'; }` swaps the display. Lowercase `[x]` per user preference.
  3. **Status visibility**: `engine-status` elements start with `display: none`. `checkEngineKeyStatus()` only called for engines where `cb.checked === true`. Checkbox `change` listener shows/hides status and re-checks storage on enable. `testAllEnabledKeys()` guards per-engine status updates with `cb.checked` check.
  4. **Help modal trigger**: Removed `?` button from primary engine row. Added `[ HOW TO GET API KEYS ]` button (same `id="engine-help-btn"`) above the engines list. Existing click handler in JS works unchanged — only location and label changed.
  5. **Custom selects**: Built a vanilla JS `createTerminalSelect(nativeSelect)` function that wraps a native `<select>` in a hidden `.terminal-select-wrap` + visible `.terminal-select` with `>` arrow. Applied to `#source-lang`, `#target-lang`, `#primary-engine`, `#overlay-position` via `data-terminal-select` attribute. Native select keeps form values. `renderPrimaryDropdown()` refreshes the primary engine wrapper after options change.
  6. **Custom title bar**: Set `decorations: false` and `resizable: false` on the `main` window in `tauri.conf.json`. Added `.window-titlebar` HTML+CSS with `[ — ]` minimize and `[ X ]` close buttons. Minimize calls `window.minimize()` (Tauri 2 API). Close calls existing `hide_window` command (window hides, not exits). Title bar is draggable via `mousedown` → `startDragging()`. Buttons exclude drag via `e.target.closest('.window-btn')` check. Existing `on_window_event` handler in `lib.rs` remains as safety net. Added `core:window:allow-minimize` to capabilities.
- **Consequences**:
  - More neutral (gray) color palette — less opinionated, wider appeal.
  - Custom selects add ~80 lines of JS but provide consistent terminal look. Profile form selects remain native (out of scope).
  - No backend changes — zero Rust code touched. All changes are CSS/HTML/JS.
  - No data migration — users on v0.9.1 keep all settings, keys, profiles, history.
  - `resizable: false` means the main window cannot be resized by the user. Content is designed for 600px width.
  - Custom title bar removes native Windows chrome — matches the terminal aesthetic of the rest of the UI.
  - No maximize button — intentional, since the settings panel is not meant to be fullscreen.
- **Alternatives considered**:
  - A1: Keep blue-dominant palette — rejected by user, didn't match the reference images.
  - A2: Use JS state management for checkbox `[x]` — rejected in favor of CSS `::before` (simpler, no JS needed).
  - A3: Convert all selects including profile form — rejected for v0.9.2 to avoid scope creep. Profile form selects are out of scope per D25.
  - A4: Keep native window decorations — rejected by user, wanted terminal-style title bar.
  - A5: Add maximize button — rejected, settings panel doesn't benefit from maximize + would require responsive layout work.

---

## ADR-019 — docs/changes/ folder for change plans

- **Date**: 2026-06-10
- **Status**: Accepted
- **Context**: The project migrated from the legacy 7-phase SDD workflow (`openspec/changes/<name>/`) to SDD Lite in commit `8a384e6`. The legacy `openspec/` folder was deleted. In the first SDD Lite changes (`settings-bugs`, `game-profile-ui-on-restart`, `instant-flow`), plans lived at the repo root as `plan.md`. As more changes accumulate, root-level plans become disorganized — no clear ownership, no archive path, no version grouping.
- **Decision**: Change plans live at `docs/changes/<change-name>/plan.md` (kebab-case change name, e.g. `ui-redesign`, `background-capture`). ADRs continue to live flat in `docs/decisions.md` (one file, append-only). The change folder is the change's workspace: it can be deleted after release if desired, or kept as historical record. Only the `plan.md` lives there for now — no other artifacts (no design.md, no tasks.md). This keeps SDD Lite's "caveman structure" — simple prompts, simple artifacts.
- **Consequences**:
  - Each change is self-contained: its plan, context, and decisions are in one folder.
  - `docs/decisions.md` stays flat and append-only — easy to scan, no nested folders.
  - Future changes follow the same pattern: `docs/changes/<name>/plan.md`.
  - Change folders can be archived (zipped, git-archived) or deleted after release without affecting other documentation.
  - Old root-level `plan.md` files (from v0.8.5, v0.8.6) remain in the repo as historical artifacts — they are not migrated retroactively.
- **Alternatives considered**:
  - A1: Keep `plan.md` at repo root (current pattern for v0.8.5/v0.8.6) — rejected because as changes accumulate, root becomes cluttered.
  - A2: Per-change folders with multiple artifacts (`plan.md`, `design.md`, `tasks.md`, `notes.md`) — rejected as over-engineered for personal projects. SDD Lite is intentionally lean.
  - A3: Folders named by type (`docs/changes/ui/`, `docs/changes/feature/`, `docs/changes/bugfix/`) — rejected because a single change can span multiple types. Change names are unambiguous.

---

## ADR-020 — Add Groq as alternative paid translation engine

- **Date**: 2026-06-10
- **Status**: Accepted
- **Context**: DeepSeek is paid (token-based) and can be cost-prohibitive for some users. Groq offers a generous free tier with comparable quality on 8B models. User wants to try Groq without removing DeepSeek. AHA principle: don't remove until 3+ use cases justify it. Additive change (G3 from analysis) is lowest-risk.
- **Decision**: Add Groq as a new paid engine with `llama-3.1-8b-instant` model. OpenAI-compatible API means adapter is ~95% identical to DeepSeek. Users opt-in by adding Groq to `enabled_engines` and providing an API key.
- **Consequences**:
  - One more option in the engine dropdown (4 paid engines total: Gemini, DeepL, DeepSeek, Groq)
  - CSP updated to allow `https://api.groq.com`
  - `test_api_key` command now supports 4 engines (was 3)
  - 429 handling on test differs from DeepSeek (reports "rate limited but key valid" instead of generic failure) — better UX for free tier users
  - 6 new unit tests in `groq.rs` (5 adapter tests + 1 engine factory test)
  - Zero settings migration (additive)
  - If Groq quality is insufficient after real-world testing, model can be swapped to `llama-3.3-70b-versatile` in a future change (different `model` field, same API)
- **Alternatives considered**:
  - G1: Replace DeepSeek with Groq — rejected because user has not yet validated Groq quality; AHA principle says don't remove without evidence
  - G2: Add Groq as free engine (no key required) — rejected because it would silently fail for users without keys, creating a poor UX footgun
  - G4: Remove DeepSeek, don't replace — rejected because it reduces user choice without adding value
