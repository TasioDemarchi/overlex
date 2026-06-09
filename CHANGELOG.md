# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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