# Plan: Add `close_with_esc` setting to disable Esc closing translation windows (v0.9.10)

## Goal

Add a new boolean setting `close_with_esc` (default `true`) that controls whether the Esc key closes the result window and the write window. When set to `false`, the Esc keydown listeners in the frontend become no-ops — the windows can only be closed via:
- The hotkey toggle (Ctrl+Shift+T for result, Ctrl+Shift+W for write)
- The (×) close button (always works, regardless of setting)
- The auto-dismiss timer (5s for result, not applicable for write)
- Translation completion (only write, when `translate_text` finishes)

This lets the user train themselves to use the hotkey toggle instead of Esc, so the Esc key stays free to interact with the game (since the toggle pattern means the user never needs to send Esc to the game while the translation window is open).

## Why this is the right design (vs disabling unconditionally)

- **Backward compatible**: default is `true` (current behavior). Users who like Esc continue to use it; users who want to force themselves to use the toggle can opt in.
- **Explicit close paths unaffected**: the (×) button and the hotkey toggle always work. The setting only affects Esc, which is a "soft" key press without explicit user intent to close the window.
- **No backend changes needed**: the listeners are in JS, the setting is in JS, no new Tauri commands.

## Files to Touch

| File | Lines | Change |
|------|-------|--------|
| `src-tauri/src/commands.rs` | 238-262 (struct) | Add `close_with_esc: bool` field with `#[serde(default = "default_true")]` |
| `src-tauri/src/commands.rs` | 300-316 (`from`) | Add `close_with_esc: s.close_with_esc` to the constructor |
| `src-tauri/src/commands.rs` | after `default_true` (line 274) | No new function needed (reuses `default_true`) |
| `src-tauri/src/commands.rs` | custom Deserialize (lines 324-360) | Add `close_with_esc: raw.close_with_esc` |
| `src-tauri/src/commands.rs` | `SettingsRaw` struct (lines 280-300) | Add `close_with_esc: bool` with `#[serde(default = "default_true")]` |
| `src/settings/index.html` | TBD — find a logical group | Add a checkbox labeled "Close with Esc" |
| `src/settings/settings.js` | settings load (around line 600-700) | Read the value, populate the checkbox |
| `src/settings/settings.js` | `save_settings` (around line 680) | Include the value in the saved settings |
| `src/result/result.js` | Esc listener (lines 301-305) | Check `__closeWithEsc` flag before closing |
| `src/result/result.js` | settings-changed listener (line 215+) | Update `__closeWithEsc` when settings change |
| `src/write/write.js` | Esc listener 1 (lines 215-217) | Check flag before closing |
| `src/write/write.js` | Esc listener 2 (lines 221-223) | Check flag before closing |
| `src/write/write.js` | settings-changed listener (line 110+) | Update flag when settings change |
| `docs/decisions.md` | end | Add ADR-028 |
| `CHANGELOG.md` | top | Add `## [0.9.10]` section |
| 3 version files | version field | Bump `0.9.9` → `0.9.10` |

## Implementation Detail

### Backend: Add field to `Settings` struct (commands.rs)

In the `Settings` struct (line 238-262), add after `show_debug` (line 261):

```rust
    #[serde(default)]
    pub show_debug: bool,
    /// Whether Esc closes the result and write windows. Default true.
    /// When false, the user must close via the hotkey toggle, the (×) button,
    /// or the auto-dismiss timer (result only).
    #[serde(default = "default_true")]
    pub close_with_esc: bool,
}
```

In `SettingsRaw` struct (line 280-300), add the same field with the same `#[serde(default = "default_true")]` annotation, so old settings files (without this field) deserialize with `close_with_esc = true`.

In the `from` impl (line 300-316), add:
```rust
            close_with_esc: s.close_with_esc,
```

In the custom Deserialize impl (lines 340-360), add:
```rust
            close_with_esc: raw.close_with_esc,
```

`default_true` already exists at line 272, no need to add a new function.

### Frontend: Settings UI

Find the appropriate place in `src/settings/index.html` near the other checkboxes. Look for a section like "Behavior" or "Window" — likely near the `start_with_windows` checkbox since it's a similar UX setting. Add a new checkbox with id `close-with-esc` and label "Close with Esc".

In `src/settings/settings.js`:
- Load: find where other booleans like `ocr_preprocessing` and `history_enabled` are loaded and add the same pattern for `close_with_esc`
- Save: add the value to the `save_settings` payload

### Frontend: Result window Esc handler

In `src/result/result.js`, declare a module-level flag at the top of the file (with the other globals like `__currentEngine`):

```javascript
let __closeWithEsc = true;  // Default true; updated on settings load + settings-changed event
```

Modify the existing Esc listener (line 301-305):

```javascript
document.addEventListener('keydown', async (e) => {
    if (e.key === 'Escape') {
        if (!__closeWithEsc) return;  // NEW: respect setting
        try { await window.__TAURI__?.core?.invoke('dismiss_result'); } catch (e) { console.error('Failed to dismiss:', e); }
    }
});
```

In the `settings-changed` listener (line 215+), add:

```javascript
if (typeof payload.close_with_esc === 'boolean') {
    __closeWithEsc = payload.close_with_esc;
}
```

### Frontend: Write window Esc handlers

In `src/write/write.js`, declare the same module-level flag:

```javascript
let __closeWithEsc = true;  // Default true; updated on settings load + settings-changed event
```

Modify both Esc listeners (lines 215-217 and 221-223):

```javascript
// Input Esc handler (inside the input keydown listener)
} else if (e.key === 'Escape') {
    if (!__closeWithEsc) return;  // NEW: respect setting
    closeWindow();
}

// Window-level Esc handler
window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        if (!__closeWithEsc) return;  // NEW: respect setting
        closeWindow();
    }
});
```

In the `settings-changed` listener (line 110+), add:

```javascript
if (typeof payload.close_with_esc === 'boolean') {
    __closeWithEsc = payload.close_with_esc;
}
```

## How it Works (Flow)

### Default behavior (close_with_esc = true, no change)
1. User opens write window via Ctrl+Shift+W
2. User presses Esc → listener checks `__closeWithEsc === true` → calls `closeWindow()` → window closes
3. **Same as today**

### New behavior (close_with_esc = false)
1. User opens Settings
2. User unchecks "Close with Esc" → saves
3. Settings emit `settings-changed` with `close_with_esc: false`
4. Both result.js and write.js update their `__closeWithEsc` to `false`
5. User opens write window via Ctrl+Shift+W
6. User presses Esc → listener checks `__closeWithEsc === false` → returns early, does nothing
7. **The Esc key now propagates to the game normally** (or to whatever window had focus)
8. User closes write window via Ctrl+Shift+W (toggle) or (×) button
9. If the user opens the window again, the setting persists (it's saved in settings.json)

### Hotkey toggle always works
- `close_with_esc = false` does NOT affect the hotkey toggle (`Ctrl+Shift+T`/`Ctrl+Shift+W` always work)
- The (×) button always works
- The auto-dismiss timer for result always works (5s default)

## Edge Cases to Verify

- **Setting persisted across app restart**: yes, it's saved to settings.json like other settings
- **Setting change while window is open**: yes, `settings-changed` event fires immediately, the in-memory flag updates, next Esc press respects it
- **User toggles the setting repeatedly**: idempotent, no side effects
- **Both result.js and write.js are independent**: each loads its own copy of the flag from the same `settings-changed` event
- **Default for new users**: `true` (current behavior preserved)
- **Default for users with old settings.json (no close_with_esc field)**: `true` (via `#[serde(default = "default_true")]` on both `Settings` and `SettingsRaw`)

## Test Plan

User must validate after building:

1. **Build and run** v0.9.10.
2. **Default behavior unchanged**: open Settings, don't change anything. Open Write with Ctrl+Shift+W, press Esc → window closes (as before).
3. **Toggle the setting**: open Settings, find "Close with Esc" checkbox, uncheck it, save.
4. **Esc no longer closes**: open Write with Ctrl+Shift+W, press Esc → window stays open, Esc propagates to game.
5. **Other close paths still work**: 
   - Press Ctrl+Shift+W → window closes (toggle still works)
   - Click (×) button → window closes
6. **Same test for result window**: trigger OCR, get result, press Esc with setting off → result stays open. Press Ctrl+Shift+T → result closes.
7. **Toggle back on**: go to Settings, check the box again, save. Open Write, press Esc → window closes (as before).
8. **Setting persists across restart**: toggle off, close app, reopen app. Open Write, press Esc → still doesn't close (setting was saved).

## Version Bump

- `src-tauri/tauri.conf.json`: `0.9.9` → `0.9.10`
- `src-tauri/Cargo.toml`: `0.9.9` → `0.9.10`
- `package.json`: `0.9.9` → `0.9.10`

## Documentation

- Add to `CHANGELOG.md`:
  ```
  ## [0.9.10] - 2026-06-12
  - feat: add "Close with Esc" setting in Settings (default true) — when disabled, Esc no longer closes the result or write windows, forcing use of the hotkey toggle or (×) button
  ```
- Add to `docs/decisions.md`:
  ```
  ## ADR-028 — Configurable Esc behavior via close_with_esc setting
  - **Context**: User wanted to disable Esc closing translation windows so they don't fall back on it. The hotkey toggle is the preferred close path (works in any game state, doesn't conflict with game Esc handlers).
  - **Decision**: Add a `close_with_esc` boolean setting (default true) that gates the Esc keydown listeners in result.js and write.js. When false, the listeners are no-ops. Other close paths (hotkey toggle, (×) button, auto-dismiss timer) are unaffected.
  - **Why**: User can opt in to the new behavior without breaking defaults. Backward compatible. Backend unchanged.
  - **Out of scope**: Per-window setting (e.g. only write, not result). Single boolean for both keeps the UI simple.
  ```

## Out of Scope (Noted, Not Fixed)

- Per-window setting (separate flag for result and write). Single boolean for both — KISS.
- Focus not restored to game on write window close (pre-existing).
- Pre-existing warnings (unused imports/variables).
- `translate_text` dead code.
- Double dispatch of `translation-result`.
