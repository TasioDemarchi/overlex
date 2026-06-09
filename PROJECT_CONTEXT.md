# Project Context: OverLex

## Project name
OverLex

## Purpose
Overlay translator for Windows games. Lets gamers translate on-screen text (OCR mode) or typed words (write mode) without Alt+Tabbing out of the game.

## Target user
Spanish-speaking gamers playing games in foreign languages (especially Japanese/English). Secondary: general desktop users who need quick translations without opening a browser.

## Stack summary

### Backend (Rust)
- **Framework**: Tauri 2 (single process, no sidecar)
- **IPC**: Tauri invoke (commands) + event system
- **Screen capture**: DXGI Desktop Duplication with GDI BitBlt fallback
- **OCR**: Windows.Media.Ocr (built-in Windows API)
- **Translation**: Multi-engine fallback chain (trait-based)
- **History**: SQLite + FTS5 via rusqlite (bundled)
- **Settings**: JSON file (%APPDATA%/overlex/settings.json) + JSON API keys file (%APPDATA%/overlex/api_keys.json)
- **Hotkeys**: Win32 RegisterHotKey with dedicated message pump thread
- **Game detection**: Background thread polling GetForegroundWindow() every 1s
- **Window effects**: window-vibrancy (acrylic blur)

### Frontend (Vanilla JS)
- **Architecture**: 4 separate webview windows (NOT a SPA)
- **Framework**: None — vanilla HTML/CSS/JS
- **Tauri bridge**: window.__TAURI__.core for invoke/events
- **State**: No frontend framework — state is fetched on demand via invoke

### Platform
- **Target**: Windows 10/11 only
- **Installer**: NSIS via Tauri bundler, distributed via GitHub Releases
- **Install script**: install.ps1 (PowerShell, downloads latest release from GitHub)

## Key commands

| Command | What it does |
|---------|-------------|
| `npm run dev` | Start Tauri dev mode (hot reload for webviews) |
| `npm run build` | Build release binary + NSIS installer |
| `npm run tauri` | Raw Tauri CLI passthrough |

On Windows, after build the installer is at `src-tauri/target/release/bundle/nsis/`.

## Source layout

```
overlex/
├── src/                          # Frontend (vanilla JS, 4 webviews)
│   ├── settings/                 # Main window: settings panel
│   │   ├── index.html
│   │   └── settings.js           # ~1088 lines, all settings + profile UI
│   ├── freeze/                   # Fullscreen screenshot overlay (OCR capture)
│   │   ├── index.html
│   │   └── freeze.js
│   ├── result/                   # Translation result overlay
│   │   ├── index.html
│   │   └── result.js
│   └── write/                    # Write mode input overlay
│       ├── index.html
│       └── write.js
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml                # Rust dependencies
│   ├── tauri.conf.json           # Tauri config (windows, CSP, bundle)
│   ├── capabilities/default.json # Tauri v2 permissions
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── lib.rs                # App setup, state, event handlers (~621 lines)
│   │   ├── commands.rs           # ALL Tauri commands (~1760 lines)
│   │   ├── settings.rs           # Settings JSON + Credential Manager
│   │   ├── capture.rs            # DXGI + GDI screen capture
│   │   ├── ocr.rs                # Windows.Media.Ocr integration
│   │   ├── hotkeys.rs            # Win32 global hotkeys
│   │   ├── history.rs            # SQLite + FTS5 history DB
│   │   ├── game_detection.rs     # Foreground window polling
│   │   ├── tray.rs               # System tray utilities
│   │   └── translation/          # Translation engine module
│   │       ├── mod.rs            # Engine trait + factory
│   │       ├── chain.rs          # Multi-engine fallback chain
│   │       ├── gemini.rs         # Gemini adapter
│   │       ├── deepseek.rs       # DeepSeek adapter
│   │       ├── deepl.rs          # DeepL adapter
│   │       ├── google_gtx.rs     # Google GTX (default, free)
│   │       └── mymemory.rs       # MyMemory (free, excluded from fallback)
│   └── tests/                    # Empty (tests are inline)
├── PRD.md                        # Product Requirements Document
├── install.ps1                   # PowerShell install script
├── package.json                  # Node deps (only @tauri-apps/cli)
└── docs/
    └── decisions.md              # Architecture Decision Records (ADRs)
```

## Critical constraints

- **Windows-only**: 100% Win32 API usage (DXGI, GDI, Windows.Media.Ocr, RegisterHotKey, Credential Manager). No cross-platform support.
- **No process injection**: Overlay is a separate process with transparent topmost windows. Does NOT interact with game memory.
- **Anti-cheat safety**: Zero game process interaction. Window flags: WS_EX_NOACTIVATE, skip taskbar, no focus steal.
- **NSIS installer only**: Distributed via GitHub Releases. No auto-update mechanism.
- **No test runner**: Inline `#[cfg(test)]` unit tests in Rust files. Run with `cargo test`. No integration/E2E tests.

## Current status

- **Version**: 0.8.2 (tauri.conf.json)
- **Status**: Post-MVP, active development
- **Default translation engine**: Google GTX (changed from LibreTranslate post-MVP)
- **Recent change**: Settings persistence fix (API keys now save to Credential Manager, base/active settings split)
- **Next feature**: Game profiles with custom AI context prompts

## Architecture diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Tauri 2 Process                              │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │                    Rust Backend                                  ││
│  │                                                                   ││
│  │  SettingsState ──┬── saved_defaults (persisted JSON)             ││
│  │                  └── settings (active, with profile overrides)    ││
│  │                                                                   ││
│  │  TranslationState ── engines: HashMap<String, Engine>            ││
│  │                   ── chain: TranslationChain (fallback order)     ││
│  │                                                                   ││
│  │  HotkeyState ──── Win32 RegisterHotKey message pump thread       ││
│  │  GameDetector ──── Polling thread (1s) → emits "game-changed"    ││
│  │  History DB ───── SQLite + FTS5 at %APPDATA%/overlex/history.db  ││
│  └─────────────────────────────────────────────────────────────────┘│
│                           │ IPC (invoke + events)                    │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │              4 Webview Windows (Vanilla JS)                      ││
│  │                                                                   ││
│  │  main (settings)  freeze (fullscreen)  result (translation)     ││
│  │  write (input)                                                    ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

## Key decisions (see docs/decisions.md for full ADRs)

| ID | Summary |
|----|---------|
| D1 | Windows-only (no Linux/macOS) |
| D2 | Frontend vanilla JS (no framework) |
| D3 | 4 separate webview windows (not SPA) |
| D4 | DXGI screen capture with GDI fallback |
| D5 | Windows.Media.Ocr for OCR |
| D6 | Multi-engine translation with fallback chain (primary → other paid → google_gtx) |
| D7 | Settings two-tiers: saved_defaults + active (profile overrides on top) |
| D8 | SQLite + FTS5 for history |
| D9 | API keys in Windows Credential Manager |
| D10 | Game detection with 1s polling |
| D11 | Overlays with acrylic blur + WS_EX_NOACTIVATE |
| D12 | In-memory log buffer |
| D13 | CSP (HAS BUG: blocks Gemini/DeepL/DeepSeek — pending fix) |
| D14 | Custom hotkey capture via Win32 |

## Known issues (next change)

1. **CSP blocks paid engines**: Current CSP only allows `*.googleapis.com` and `api.mymemory.translated.net`, blocking Gemini, DeepL, and DeepSeek API calls.
2. **API keys not persisting (post v0.8.2)**: Although the settings-persistence fix was released, testing shows keys still don't survive restarts in v0.8.2.
3. **Game profiles not hydrated on startup**: Game profiles are loaded from saved_defaults but not applied to active settings on initial app launch — they only apply when a "game-changed" event fires.

## Related skills

- No Tauri-specific skill available in the skill registry.
- No Windows-specific skill available.
- Rust/Rust testing patterns are general (use standard Rust practices).

## Workflow mode
COLLABORATIVE — user validates all decisions and plans before implementation.
