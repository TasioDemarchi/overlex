# Plan: OCR hotkey toggle — re-press closes the result window (v0.9.6)

## Goal

Add a UX improvement: if the result window is already visible when the user presses the OCR hotkey (`Ctrl+Shift+T`), the second press **closes the window instead of starting a new OCR flow**. This solves the practical Esc issue from a different angle: the user already has `Ctrl+Shift+T` in muscle memory, so re-pressing it to close the overlay feels natural and avoids the need for any Esc interception or low-level keyboard hooks.

The existing Esc listener in `result.js:301` stays as-is — it works when the window has focus, and we accept that Esc may also reach the game (typical game design: Esc closes dialogs, that's fine).

## Why this approach (over a low-level keyboard hook)

- **No DLL, no hook, no anti-cheat risk** — `RegisterHotKey` is already proven to work (we use it for OCR, Write, Swap).
- **Reuses existing hotkey infrastructure** — no new thread, no new module.
- **User already has the keybinding in muscle memory** — no new shortcut to learn.
- **KISS** — the v0.9.5 WS_EX_NOACTIVATE toggling logic was a half-fix; this is the complete, clean UX solution.

## Files to Touch (Least Touch)

| File | Lines | Change |
|------|-------|--------|
| `src-tauri/src/hotkeys.rs` | 218-221 | Add visibility check before emitting `start-ocr-flow`; if visible, call `dismiss_result` and return early |
| `src-tauri/src/hotkeys.rs` | 1-20 | Add `IsWindowVisible` import + a small helper `is_result_window_visible(app: &AppHandle) -> bool` |
| `docs/decisions.md` | end | Add ADR-025 documenting this UX decision |
| `CHANGELOG.md` | top | Add `## [0.9.6]` section |
| 3 version files | version field | Bump `0.9.5` → `0.9.6` |

No changes to:
- `commands.rs` — `dismiss_result` already does the right thing (hide + re-apply WS_EX_NOACTIVATE)
- Any frontend file
- `tauri.conf.json` (except version)
- `Cargo.toml` (except version)

## Implementation Detail

### Helper: `is_result_window_visible` (new, in `hotkeys.rs`)

`WebviewWindow::is_visible()` in Tauri v2 is unreliable for windows with `WS_EX_NOACTIVATE` — it tracks Tauri-side visibility, not actual OS-level window state. The reliable way on Windows is `IsWindowVisible` from the Win32 API, which checks both the `WS_VISIBLE` style and whether any ancestor windows are visible.

Add this helper near the top of `hotkeys.rs` (after the `use` statements):

```rust
/// Check if the result window is currently visible at the OS level.
/// Uses IsWindowVisible because WebviewWindow::is_visible() is unreliable
/// for windows with WS_EX_NOACTIVATE (it tracks Tauri-side state, not OS state).
#[cfg(target_os = "windows")]
fn is_result_window_visible(app_handle: &tauri::AppHandle) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    if let Some(window) = app_handle.get_webview_window("result") {
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
fn is_result_window_visible(_app_handle: &tauri::AppHandle) -> bool {
    false
}
```

### Modify the OCR hotkey handler (hotkeys.rs lines 218-221)

Current code:
```rust
HOTKEY_ID_OCR => {
    app_log!("OCR hotkey pressed!");
    let _ = app_handle.emit("start-ocr-flow", ());
}
```

New code:
```rust
HOTKEY_ID_OCR => {
    app_log!("OCR hotkey pressed!");
    // Toggle: if the result window is already visible, close it instead
    // of starting a new OCR flow. This gives the user a single hotkey
    // for both open and close, which avoids the Esc-leaks-to-game problem.
    if is_result_window_visible(&app_handle) {
        app_log!("Result window already visible — dismissing via OCR hotkey toggle");
        if let Some(window) = app_handle.get_webview_window("result") {
            let _ = window.hide();
            // Re-apply WS_EX_NOACTIVATE on hide (dismiss_result does this,
            // but we call hide() directly here to avoid a re-entrant command call)
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GetWindowLongPtrW, GWL_EXSTYLE};
                use windows::Win32::Foundation::HWND;
                if let Ok(hwnd) = window.hwnd() {
                    let hwnd = HWND(hwnd.0);
                    unsafe {
                        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | 0x08000000_isize);
                    }
                }
            }
        }
        return;
    }
    let _ = app_handle.emit("start-ocr-flow", ());
}
```

**Design decision on the inline WS_EX_NOACTIVATE re-application**: We could call `app_handle.emit("dismiss_result", ())` instead, but that re-enters the command via the IPC layer, which is wasteful and adds latency. We could also refactor `dismiss_result` to expose a sync helper, but that's scope creep. The inline 10 lines keep `Least Touch`. The duplication of the WS_EX_NOACTIVATE code is acceptable for this small fix — it's the same pattern already in `commands.rs:1141`.

**Alternative considered but rejected**: Move the WS_EX_NOACTIVATE logic into the helper that already exists (`set_result_window_noactivate` in `commands.rs`). Rejected because it would require either making that function `pub` and `pub(crate)`, or duplicating it in `hotkeys.rs`. Both are more disruptive than the inline re-application.

## How it Works (Flow)

1. **User presses `Ctrl+Shift+T`** → OCR hotkey fires.
2. **Handler checks `is_result_window_visible()`**:
   - **`false`** (normal case, no window open) → emits `start-ocr-flow` → existing flow runs → result window appears.
   - **`true`** (window already visible) → hides the window + re-applies `WS_EX_NOACTIVATE` → returns early. No new OCR triggered.
3. **User can now close the result window with the SAME hotkey** that opened it.

## Edge Cases to Verify

- **Window exists but is hidden** (`visible: false` from `tauri.conf.json`, never shown yet): `IsWindowVisible` returns `false` → new OCR fires. Correct.
- **Window exists, currently shown by error path** (`emit_error`): same as above, `IsWindowVisible` returns `true` → toggle closes it. Correct.
- **Window destroyed**: `get_webview_window` returns `None` → helper returns `false` → new OCR fires. Safe.
- **Auto-dismiss timer fires (5s)**: timer calls `dismiss_result` → window hides normally. Next OCR hotkey press → window is hidden → new OCR fires. Correct.
- **User presses OCR hotkey rapidly during OCR processing** (OCR is in flight, freeze window visible, not result window yet): `IsWindowVisible` for result window returns `false` (it's still hidden) → new OCR fires, may restart the flow. **Pre-existing behavior, not our problem.** The freeze window blocks input via its own modal canvas, so this race is essentially impossible.
- **Multiple result windows somehow** (shouldn't happen, but): helper checks the labeled window "result" specifically. Safe.

## Test Plan

User must validate after building:

1. **Build and run** v0.9.6.
2. **Trigger OCR** (`Ctrl+Shift+T`) with a game dialog open → translation appears.
3. **Press `Ctrl+Shift+T` again** → translation window closes. **No new OCR starts.** Game dialog stays open.
4. **Press `Ctrl+Shift+T` a third time** → new OCR starts, new translation appears.
5. **Press Esc on the visible window** → window closes (Esc still works as before, may also close game dialog — that's accepted).
6. **Wait 5s for auto-dismiss** → window hides. Next `Ctrl+Shift+T` starts new OCR.
7. **Test with error state**: trigger an error (e.g. invalid API key) → error window shows. Press `Ctrl+Shift+T` → error window closes, no new OCR.

## Version Bump

- `src-tauri/tauri.conf.json`: `0.9.5` → `0.9.6`
- `src-tauri/Cargo.toml`: `0.9.5` → `0.9.6`
- `package.json`: `0.9.5` → `0.9.6`

## Documentation

- Add to `CHANGELOG.md`:
  ```
  ## [0.9.6] - 2026-06-11
  - feat: OCR hotkey (Ctrl+Shift+T) now toggles the result window — press it again to close the open translation overlay
  ```
- Add to `docs/decisions.md`:
  ```
  ## ADR-025 — OCR hotkey toggles result window visibility
  - **Context**: The Esc key on the result window was leaking to the game, closing in-game dialogs. v0.9.5 attempted to fix this by toggling WS_EX_NOACTIVATE but the keydown still propagated to the game.
  - **Decision**: Re-pressing the OCR hotkey (Ctrl+Shift+T) while the result window is visible closes the window instead of starting a new OCR flow. Esc listener stays as-is.
  - **Why**: Reuses the existing RegisterHotKey infrastructure, no DLL/hook needed, no anti-cheat risk, the key is already in the user's muscle memory.
  ```

## Out of Scope (Noted, Not Fixed)

- `translate_text` command is dead code (registered but never called). Cleanup for a future change.
- Double dispatch of `translation-result` (event + eval) can re-trigger `onTranslationResult`. Pre-existing, not our problem.
- The v0.9.5 WS_EX_NOACTIVATE toggling code stays. It still has value: it allows the Esc listener to fire when the window has focus, and the dismiss_result re-application ensures the flag is properly reset on close. The v0.9.6 toggle is a complementary UX, not a replacement.
