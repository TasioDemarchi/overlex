# OverLex — Product Requirements Document

> **Version**: 1.1
> **Date**: 2026-06-09 (updated)
> **Status**: Post-MVP, active development

---

## 1. Vision

OverLex is a lightweight Windows desktop overlay that provides instant text translation without leaving your current application. Designed primarily for gamers playing in borderless windowed mode, it eliminates the need to Alt+Tab just to look up a word or phrase.

**One-liner**: Translate anything on your screen without ever leaving your game.

---

## 2. Problem Statement

Gamers who play in a non-native language constantly face a friction point: encountering unknown words or phrases forces them to minimize or Alt+Tab out of the game to use a translator. This breaks immersion, wastes time, and discourages players from engaging with games in their original language.

Beyond gaming, any user working with foreign-language content (documents, websites, videos) faces the same context-switching overhead.

---

## 3. Target Users

### Primary: Gamers
- Play in borderless windowed mode
- Want to understand game text (menus, dialogs, quests, chat) without leaving the game
- Motivated to play in the original language but need a safety net for unknown words

### Secondary: General desktop users
- Students, professionals, or anyone who encounters foreign text on screen
- Want quick translations without opening a browser tab

---

## 4. Core Features (MVP)

### 4.1 OCR Capture Mode
- **Trigger**: Global hotkey (user-configurable)
- **Flow**:
  1. Hotkey → instant fullscreen screenshot captured in background
  2. Frozen screenshot displayed as fullscreen overlay (slight dim) → game pauses visually but keeps running underneath
  3. User draws selection rectangle on the frozen image (click + drag)
  4. On mouse release → freeze overlay DISAPPEARS immediately → user is back in the live game
  5. Small result overlay shows "Translating..." while OCR + translation happen in background
  6. Result overlay updates with: original text → translated text
  7. Overlay auto-dismisses after timeout or manual dismiss
- **Requirements**:
  - Must work over any application in borderless windowed / windowed mode
  - Freeze phase must be as brief as possible (only during selection, typically 1-2 seconds)
  - Game continues running after selection — user is never locked out of gameplay while waiting for translation
  - Selection tool must be intuitive and fast (click + drag)
  - OCR must handle common game fonts reasonably well
  - Source language auto-detected or manually set

### 4.2 Write Mode
- **Trigger**: Different global hotkey (user-configurable)
- **Flow**: Hotkey → floating input field appears → user types word/phrase → translation shown in real-time or on Enter → result shown in overlay
- **Requirements**:
  - Input field must be minimal and non-intrusive
  - Must not capture game input while active (grab focus cleanly)
  - Support pressing Escape to dismiss

### 4.3 Translation Overlay
- **Behavior**: Transparent, always-on-top window
- **Shows**: Original text + translated text
- **Dismissal**: Auto-dismiss after configurable timeout, or manual dismiss via hotkey/click
- **Position**: Configurable (corner of screen or near selection area)

### 4.4 Translation Engine
- **Default (current)**: Google GTX (free, no API key) — changed from LibreTranslate post-MVP
- **Fallback chain**: primary engine → other enabled paid engines → Google GTX (last resort). MyMemory is excluded from fallback chain (only used as primary).
- **Why cloud**: Offline engines (Argos Translate) consume 100MB+ RAM when loaded, violating the < 50MB constraint for mid/low-end PCs
- **Extensible**: Architecture supports plugging in additional engines via trait-based TranslationEngine pattern
- **Supported engines**: Google GTX (default, free), MyMemory (free), Gemini (API key), DeepL (API key), DeepSeek (API key)
- **AI context**: Gemini, DeepSeek, and DeepL accept game context (process name, profile name, custom prompt) for domain-aware translation
- **Initial languages**: Spanish <-> English (bidirectional), with support for 25+ languages
- **Future**: Offline engine as optional mode (with RAM trade-off warning to user)

### 4.5 Settings
- Configurable hotkeys for each mode
- Source and target language selection
- Translation engine selection (engine + enabled engine list)
- API key input for premium engines (optional, stored in Windows Credential Manager)
- Overlay position and auto-dismiss timeout
- Start minimized / start with Windows (optional)
- Game profiles (see 4.6)
- Show debug toggle (in-memory log viewer)

### 4.6 Game Profiles with AI Context
- **Purpose**: Each game profile stores overrides for language, engine, and OCR settings that auto-apply when the game is detected. Additionally, profiles can include a custom AI prompt that gives context about the game to AI-powered engines (Gemini, DeepSeek, DeepL).
- **Auto-detection**: Background polling detects the foreground window process name every 1s and matches against configured game profiles.
- **Profile overrides**: Profile fields (source_lang, target_lang, primary_engine, ocr options) override the saved defaults when the game is active. Defaults are never modified.
- **Custom AI prompt**: Each profile can include a free-text prompt describing the game's lore, characters, terminology, and translation preferences. This prompt is sent to AI engines as system context, improving translation quality for game-specific terms.
- **Fallback on no match**: When no game profile matches, the active settings revert to saved defaults (no stale overrides).

---

## 5. Technical Constraints

| Constraint | Detail |
|------------|--------|
| **Platform** | Windows 10/11 only (MVP) |
| **Overlay method** | Transparent topmost window — NO process injection |
| **Anti-cheat safety** | Runs as a separate process; does not interact with game memory |
| **Performance** | Must run smoothly on mid/low-end PCs without affecting game FPS. Target: < 50MB RAM idle, < 1% CPU idle, < 5% CPU during OCR capture (brief spike). No background polling or continuous screen capture |
| **Target hardware** | Mid/low-end gaming PCs (e.g. 8GB RAM, integrated or entry-level GPU). OverLex must never compete with the game for resources |
| **OCR** | Windows built-in OCR API (Windows.Media.Ocr) — zero extra size, pre-installed on Windows 10/11. Requires target language pack installed in Windows settings |
| **Default translation** | Google GTX (free, no API key) with adaptive fallback chain. Offline mode as future option with higher RAM trade-off |

---

## 6. Out of Scope (MVP)

- Translation history / vocabulary tracker [OBSOLETO: History with SQLite + FTS5 was implemented post-MVP]
- Gamification or learning features
- Multi-platform (macOS, Linux)
- Fullscreen exclusive game support
- Anti-cheat compatibility list
- Premium/paid features
- Auto-update mechanism
- Multiplayer chat real-time translation

---

## 7. User Flows

### 7.1 OCR Capture Flow
```
User playing game
  → Presses Ctrl+Shift+T (example)
  → Instant fullscreen screenshot captured
  → Frozen image displayed as dimmed fullscreen overlay
  → User clicks and drags over text region on frozen image
  → Releases mouse
  → Freeze overlay DISAPPEARS → game is live again
  → Small overlay appears: "Translating..."
  → OCR extracts text → sent to cloud translation API
  → Overlay updates with result:
      "Quest accepted" → "Misión aceptada"
  → Overlay auto-dismisses after 5 seconds (or user presses Esc)
```

### 7.2 Write Mode Flow
```
User playing game
  → Presses Ctrl+Shift+W (example)
  → Small floating input appears (centered or corner)
  → User types: "surrender"
  → Presses Enter
  → Overlay shows: "surrender" → "rendirse"
  → User presses Esc or overlay auto-dismisses
  → User continues playing
```

---

## 8. Success Metrics

For a v1.0 personal release:
- OCR correctly recognizes text in 80%+ of captures on common game fonts
- Translation response time < 2 seconds (OCR mode), < 500ms (write mode)
- Zero perceptible FPS impact on mid/low-end PCs (8GB RAM, entry-level GPU)
- App memory usage < 50MB idle, < 80MB during OCR capture
- CPU usage < 1% idle, < 5% during brief OCR spike
- No background processes polling or scanning the screen

---

## 9. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| OCR fails on stylized game fonts | Core feature broken | Allow manual language hints; consider multiple OCR engines |
| Free translation quality is poor | Bad UX | Architecture allows swapping engines; DeepL/Google as optional upgrade |
| Anti-cheat false positives | Users banned | Document that it's a separate process; maintain compatibility list over time |
| Overlay steals game focus | Gameplay disrupted | Careful window flag management (no-focus, click-through where appropriate) |
| User has 0 dev experience | Project stalls | Guided step-by-step development with clear milestones |

---

## 10. Future Considerations (Post-MVP)

- Translation history with search [DONE: SQLite + FTS5 history implemented in v0.8.x]
- Advanced AI context prompts per game profile [IN PROGRESS: per-profile custom prompts for Gemini/DeepSeek/DeepL]
- Vocabulary tracker / flashcard integration
- Additional language pairs
- Premium tier with advanced engines or features
- Game compatibility database (community-driven)
- Linux / macOS support via Tauri cross-platform
- Real-time chat translation for multiplayer games
- Custom OCR training for specific game fonts
- Plugin system for community extensions

---

## 11. Monetization Strategy

### Phase 1 (MVP): Free
- Core OCR and Write mode translation
- Free cloud translation API
- Open source (TBD)

### Phase 2 (Future): Freemium
- Free tier: all MVP features
- Premium tier (monthly/one-time): premium translation engines included, advanced features (history, vocabulary, priority support)

---

## 12. Tech Stack (Decided)

| Component | Technology | Reason |
|-----------|-----------|--------|
| **Framework** | Tauri 2 | Lightweight (~20-30MB RAM, <10MB disk), native Windows access, web frontend |
| **Frontend** | HTML/CSS/JS (Vanilla) | Overlay UI, settings panel — simple and beginner-friendly |
| **Backend** | Rust (Tauri core) | Global hotkeys, window management, screen capture, OCR integration |
| **OCR** | Windows OCR API (Windows.Media.Ocr) | Built-in, zero bloat, fast, handles game fonts well |
| **Translation (default)** | Google GTX (free, no API key) | 0 RAM footprint, < 500ms response, no registration required |
| **Translation (paid)** | Gemini, DeepL, DeepSeek (API key required) | Higher quality, accept game context for domain-aware translation |
| **Translation (fallback)** | Adaptive chain: primary → other paid → Google GTX | MyMemory excluded from fallback (only as primary) |
| **Installer** | Tauri bundler (NSIS) | Native Windows installer |

> Tech stack confirmed during exploration phase (2026-04-17).
