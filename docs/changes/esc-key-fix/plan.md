# Plan: Esc Key Fix — result window must consume Esc, not leak to game

## Goal

Fix the bug where pressing **Esc** to close the OverLex translation overlay also closes in-game dialogs/tutorials, breaking gameplay flow. The root cause is that the result window has `WS_EX_NOACTIVATE` set, which prevents it from receiving keyboard focus — so the Esc keydown never reaches the existing `keydown` listener in `src/result/result.js:301-305`, and the key propagates straight to the game in the background.

## Approach (Option C — KISS)

Allow the result window to receive keyboard focus **only while visible**, by removing the `WS_EX_NOACTIVATE` extended style just before showing it, and re-applying it when the window is hidden. This is the minimal change: no DLL hooks, no new hotkey registrations, no new files, no new Tauri commands.

The existing `keydown` listener in `result.js:301-305` already calls `dismiss_result` on Esc — once the window can receive focus, that listener will work as intended.

**Why not Option A (low-level keyboard hook):** Overkill for a single key. Requires extra module + DLL injection.
**Why not Option B (RegisterHotKey for Esc globally):** Esc as a global hotkey is aggressive and can conflict with other apps. We try the simplest path first.
**Why this is safe:** The result window currently does NOT call `set_focus()` after `.show()` (unlike freeze/write windows), so removing `WS_EX_NOACTIVATE` does NOT cause the window to actively steal focus from the game. It just allows the window to *receive* keyboard input if it ever gets focus naturally — which is exactly what we need for the Esc listener to fire.

## Files to Touch (Least Touch)

| File | Lines | Change |
|------|-------|--------|
| `src-tauri/src/commands.rs` | 136-151 (`emit_error`) | Remove `WS_EX_NOACTIVATE` before `.show()` (if currently set) |
| `src-tauri/src/commands.rs` | 155-170 (`emit_result`) | Remove `WS_EX_NOACTIVATE` before `.show()` (if currently set) |
| `src-tauri/src/commands.rs` | 1106-1113 (`dismiss_result`) | Re-apply `WS_EX_NOACTIVATE` after `.hide()` |

No changes to:
- `lib.rs` (the setup-time `WS_EX_NOACTIVATE` application at line 603 stays — it just becomes the **default state** that we toggle off temporarily)
- `src/result/result.js` (existing Esc listener at line 301 is the entire fix on the frontend)
- `tauri.conf.json`
- `Cargo.toml` (windows crate already at 0.58)

## Implementation Detail

### Helper function (private, inside `commands.rs`)

Add a small private helper that toggles the `WS_EX_NOACTIVATE` bit on the result window's extended style. Place it near `emit_result`/`emit_error` (around line 130, before `emit_error`).

```rust
/// Toggle the WS_EX_NOACTIVATE flag on the result window.
/// When `enable` is true, the flag is set (default state — window won't receive focus).
/// When `enable` is false, the flag is cleared (window can receive focus, e.g. for Esc).
#[cfg(target_os = "windows")]
fn set_result_window_noactivate(result_window: &tauri::WebviewWindow, enable: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GetWindowLongPtrW, GWL_EXSTYLE};
    use windows::Win32::Foundation::HWND;

    if let Ok(hwnd) = result_window.hwnd() {
        let hwnd = HWND(hwnd.0);
        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            // WS_EX_NOACTIVATE = 0x08000000
            let new_style = if enable {
                ex_style | 0x08000000_isize
            } else {
                ex_style & !0x08000000_isize
            };
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn set_result_window_noactivate(_result_window: &tauri::WebviewWindow, _enable: bool) {
    // No-op on non-Windows platforms
}
```

### Modify `emit_error` (commands.rs line 136-151)

In the `if show_window` branch, call `set_result_window_noactivate(&result_window, false);` **before** `result_window.show();`.

```rust
if show_window {
    set_result_window_noactivate(&result_window, false);
    let _ = result_window.show();
}
```

### Modify `emit_result` (commands.rs line 155-170)

Same pattern as `emit_error`:

```rust
if show_window {
    set_result_window_noactivate(&result_window, false);
    let _ = result_window.show();
}
```

### Modify `dismiss_result` (commands.rs line 1106-1113)

After `window.hide()`, re-apply the flag:

```rust
#[tauri::command]
pub async fn dismiss_result(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("result") {
        window.hide().map_err(|e| e.to_string())?;
        set_result_window_noactivate(&window, true);
    }
    Ok(())
}
```

## How it Works (Flow)

1. **App startup** → `lib.rs:603` sets `WS_EX_NOACTIVATE` on result window (default, unchanged).
2. **OCR completes** → `emit_result` is called → `set_result_window_noactivate(false)` clears the flag → `.show()` makes the window visible → window can now receive focus → existing `result.js:301` keydown listener is active.
3. **User presses Esc** → keydown event reaches the result window → listener calls `dismiss_result` → window hides → `set_result_window_noactivate(true)` re-applies the flag → next show will re-clear it.
4. **Auto-dismiss timer fires** (5s) → same `dismiss_result` is called → flag is re-applied. No change.
5. **Click on (×) button** → same `dismiss_result` is called → flag is re-applied. No change.

## Edge Cases to Verify

- **Rapid show/hide**: If user triggers OCR multiple times in quick succession, the toggle happens each time. Safe — it's just bit manipulation on the extended style.
- **Error case (`emit_error`)**: The result window is shown for errors too. The Esc fix applies to errors as well — good.
- **Window destroyed/recreated**: If Tauri ever destroys the result window, `get_webview_window` returns `None` and we skip the toggle. Safe.
- **No `set_focus()` is added**: We do NOT add `set_focus()` calls. The window may or may not get focus naturally, but the Esc keydown will be delivered to it as long as it's the foreground window OR if Tauri/WebView2 routes keystrokes to visible windows. **This needs empirical testing** — see Test Plan.

## Test Plan

Since I cannot build/run the project, the user must validate:

1. **Build and run** the app.
2. **Trigger an OCR translation** in a game with a dialog open.
3. **Press Esc** while the result window is visible.
   - **Expected**: result window closes, in-game dialog stays open.
   - **If it works**: success, ship.
   - **If Esc still doesn't reach the window** (e.g. game has captured Esc): this is a deeper issue — fall back to Option B.
4. **Trigger another OCR**, wait 5s for auto-dismiss.
   - **Expected**: window disappears, next OCR shows window correctly.
5. **Click the (×) button** on the result window.
   - **Expected**: window closes, next OCR works.
6. **Trigger an error** (e.g. invalid API key) and verify the error window also closes with Esc.

## Version Bump

This is a user-facing bug fix. Bump version in:
- `src-tauri/tauri.conf.json` (version field)
- `src-tauri/Cargo.toml` (version field)
- `package.json` (version field)

From current `0.9.4` → `0.9.5`.

## Documentation

- Add entry to `CHANGELOG.md` under `## [0.9.5]`:
  ```
  - fix: Esc key now closes the translation overlay without leaking to the game
  ```
- Add a one-line ADR to `docs/decisions.md`:
  ```
  ## ADR-012 — Result window toggles WS_EX_NOACTIVATE per show/hide
  - **Context**: Esc on the result window must close the overlay but not the in-game dialog.
  - **Decision**: Clear WS_EX_NOACTIVATE before .show(), re-apply after .hide() (commands.rs).
  - **Why**: Window can receive keyboard focus only while the flag is off. We don't call set_focus(), so the game keeps focus naturally.
  ```

## Out of Scope (Noted, Not Fixed)

- `translate_text` command is dead code (registered but never called). Cleanup for a future change.
- Double dispatch of `translation-result` (event + eval) can re-trigger `onTranslationResult`. Pre-existing, not our problem here.
