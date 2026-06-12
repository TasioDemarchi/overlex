# Plan: Write hotkey toggle — re-press closes the write window (v0.9.7)

## Goal

Mirror the v0.9.6 OCR hotkey toggle for the Write hotkey (`Ctrl+Shift+W` by default). If the write window is already visible when the user presses the Write hotkey again, the second press **closes the window instead of starting a new write flow**.

This gives the user a single hotkey for both open and close on both translation modes (OCR and Write), matching the v0.9.6 UX.

## Why this is analogous (but different) to v0.9.6

| | OCR toggle (v0.9.6) | Write toggle (v0.9.7) |
|---|---|---|
| **Window to check** | `result` | `write` |
| **Window to hide** | `result` | `write` |
| **Has `WS_EX_NOACTIVATE`?** | Yes (toggle clears it) | No (window takes focus actively) |
| **Helper function** | `is_result_window_visible()` | `is_write_window_visible()` |
| **WS_EX_NOACTIVATE re-apply on hide?** | Yes (inline, 10 lines) | **No** (write window doesn't have the flag) |

The structure is identical, just simpler (no WS_EX_NOACTIVATE dance needed because the write window doesn't have that flag).

## Files to Touch (Least Touch)

| File | Lines | Change |
|------|-------|--------|
| `src-tauri/src/hotkeys.rs` | 26-46 (new) | Add `is_write_window_visible` helper, identical to `is_result_window_visible` but checks "write" label |
| `src-tauri/src/hotkeys.rs` | 270-274 (current) | Add visibility check before emitting `start-write-flow`; if visible, hide the write window and return early |
| `docs/decisions.md` | end | Add ADR-026 |
| `CHANGELOG.md` | top | Add `## [0.9.7]` section |
| 3 version files | version field | Bump `0.9.6` → `0.9.7` |

No changes to:
- `commands.rs` — no Tauri command needed
- Any frontend file
- `tauri.conf.json` (except version)
- `Cargo.toml` (except version)
- `lib.rs` — the `start-write-flow` listener stays untouched; we just don't emit the event when toggling

## Implementation Detail

### Helper: `is_write_window_visible` (new, in `hotkeys.rs`)

Add immediately after `is_result_window_visible` (around line 46), using the same pattern:

```rust
/// Check if the write window is currently visible at the OS level.
/// Uses IsWindowVisible because WebviewWindow::is_visible() is unreliable
/// for windows with WS_EX_NOACTIVATE (it tracks Tauri-side state, not OS state).
/// (The write window doesn't currently have WS_EX_NOACTIVATE, but we use
/// IsWindowVisible for consistency and future-proofing.)
#[cfg(target_os = "windows")]
fn is_write_window_visible(app_handle: &tauri::AppHandle) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    if let Some(window) = app_handle.get_webview_window("write") {
        if let Ok(hwnd) = window.hwnd() {
            let hwnd = HWND(hwnd.0);
            unsafe { IsWindowVisible(hwnd).as_bool() }
        } else {
            false
        }
    } else {
        false
    }
}

#[cfg(not(target_os = "windows"))]
fn is_write_window_visible(_app_handle: &tauri::AppHandle) -> bool {
    false
}
```

### Modify the Write hotkey handler (hotkeys.rs lines 270-274)

Current code:
```rust
HOTKEY_ID_WRITE => {
    app_log!("Write hotkey pressed!");
    let _ = app_handle.emit("start-write-flow", ());
}
```

New code:
```rust
HOTKEY_ID_WRITE => {
    app_log!("Write hotkey pressed!");
    // Toggle: if the write window is already visible, close it instead
    // of starting a new write flow. Mirrors the v0.9.6 OCR hotkey toggle.
    if is_write_window_visible(&app_handle) {
        app_log!("Write window already visible — dismissing via Write hotkey toggle");
        if let Some(window) = app_handle.get_webview_window("write") {
            let _ = window.hide();
        }
        return;
    }
    let _ = app_handle.emit("start-write-flow", ());
}
```

**Why simpler than OCR toggle**: the write window does NOT have `WS_EX_NOACTIVATE`, so we don't need to re-apply any flag on hide. Just `window.hide()` and we're done. The frontend's `closeWindow()` in `write.js:37-41` is NOT called (which would also reset the chat history) — the toggle uses Rust's `hide()` directly to keep the implementation simple. The chat will be reset the next time the user opens the write window, because that's what `closeWindow()` does anyway. Wait — actually, `closeWindow()` is only called by the JS-side close handlers (Esc, X button), NOT by Rust's `hide()`. So toggling close via Rust keeps the chat history intact across open/close cycles, which is actually a small UX improvement.

**Tradeoff to be aware of**: if the user had text in the input or chat history, it persists across toggle close/reopen (because we don't call `closeWindow()`). When the user closes via Esc or X button, the chat is reset. So the toggle path has slightly different behavior from the other close paths. This is acceptable because:
- It's consistent with v0.9.6 OCR toggle (which also doesn't reset anything)
- Users who care about chat history can use Esc or X to close (which resets)
- The chat reset is a side effect of `closeWindow()`, not a feature

## How it Works (Flow)

1. **User presses `Ctrl+Shift+W`** → Write hotkey fires.
2. **Handler checks `is_write_window_visible()`**:
   - **`false`** (normal case, no window open) → emits `start-write-flow` → existing flow runs (`lib.rs:509-549` shows the write window, focuses it for input).
   - **`true`** (window already visible) → hides the window → returns early. No new write flow triggered.
3. **User can now close the write window with the SAME hotkey** that opened it.
4. **Focus behavior**: When hidden via toggle, the focus is NOT restored to the previous foreground window (this is a pre-existing limitation also present with Esc/X manual close, accepted as a known minor bug).

## Edge Cases to Verify

- **Window exists but is hidden** (`visible: false` from `tauri.conf.json`, never shown yet): `IsWindowVisible` returns `false` → new write flow starts. Correct.
- **Window exists, currently shown**: `IsWindowVisible` returns `true` → toggle closes it. Correct.
- **Window destroyed**: `get_webview_window` returns `None` → helper returns `false` → new write flow starts. Safe.
- **Result window is visible (from a previous OCR) but write window is not**: `is_write_window_visible` returns `false` → new write flow starts. The result window stays visible. Correct — they're independent.
- **User types text, then toggles close, then reopens**: Text in the input persists (Rust `hide()` doesn't reset JS state). The chat history is preserved across toggle close/reopen. On the next Esc/X close, the chat is reset by `closeWindow()`.
- **Focus not restored**: As noted, toggling close leaves focus on the hidden write window. The user must click on the game to refocus. Same as Esc/X manual close. Accepted as a known minor UX issue for a future fix.

## Test Plan

User must validate after building:

1. **Build and run** v0.9.7.
2. **Trigger write** (`Ctrl+Shift+W`) with a game open → write window appears, takes focus.
3. **Press `Ctrl+Shift+W` again** → write window closes. No new write flow starts.
4. **Press `Ctrl+Shift+W` a third time** → new write window opens.
5. **Test Esc and X button** still work (and reset chat history).
6. **Test interaction with result window**: trigger OCR, get a result window, then trigger write → write window opens on top. Press `Ctrl+Shift+W` → write closes, result window still visible. Correct.
7. **Test the chat reset behavior**: open write, type some text, press `Ctrl+Shift+W` to close, press `Ctrl+Shift+W` again to reopen → text should still be there (Rust `hide()` doesn't reset). Then type, press Esc to close, press `Ctrl+Shift+W` to reopen → text is reset (because Esc called `closeWindow()`).

## Version Bump

- `src-tauri/tauri.conf.json`: `0.9.6` → `0.9.7`
- `src-tauri/Cargo.toml`: `0.9.6` → `0.9.7`
- `package.json`: `0.9.6` → `0.9.7`

## Documentation

- Add to `CHANGELOG.md`:
  ```
  ## [0.9.7] - 2026-06-12
  - feat: Write hotkey (Ctrl+Shift+W by default) now toggles the write window — press it again to close the open write overlay
  ```
- Add to `docs/decisions.md`:
  ```
  ## ADR-026 — Write hotkey toggles write window visibility
  - **Context**: After v0.9.6 OCR toggle worked, the user requested the same UX for the Write hotkey.
  - **Decision**: Re-pressing the Write hotkey (Ctrl+Shift+W) while the write window is visible closes it instead of starting a new write flow.
  - **Why**: Consistency with v0.9.6, reuses existing helper pattern (is_write_window_visible), no new infrastructure needed.
  - **Tradeoff accepted**: Toggle close uses Rust's window.hide() directly, not the frontend's closeWindow(), so chat history is preserved across toggle cycles (only Esc/X reset it). This is consistent with v0.9.6 OCR behavior.
  - **Known limitation**: Focus is not restored to the previous foreground window on toggle close (same as Esc/X manual close, pre-existing).
  ```

## Out of Scope (Noted, Not Fixed)

- Focus not restored to game on write window close (pre-existing bug, also affects Esc/X manual close). Fix in a future change.
- Pre-existing warnings in commands.rs and capture.rs (unused variables/imports). Not our problem.
- `translate_text` command is dead code. Cleanup for a future change.
- Double dispatch of `translation-result` (event + eval). Pre-existing.
