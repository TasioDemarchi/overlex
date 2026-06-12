# Plan: Replace IsWindowVisible with manual AtomicBool state tracking (v0.9.8)

## Goal

Fix the v0.9.7 bug where the Write hotkey toggle only works the first time. The root cause: `IsWindowVisible` (and equivalently `WebviewWindow::is_visible()`, which delegates to `IsWindowVisible` on Windows) returns inconsistent results for the write window after a `hide()` operation — it keeps reporting `true` even when the window is logically hidden, so the toggle's visibility check always hits the "already visible" branch and the 3rd press never emits `start-write-flow`.

Replace the unreliable Win32 visibility check with a **manual `AtomicBool` state tracker** that we set to `true` when showing a window and `false` when hiding it, from the same code path that performs the show/hide. This is the canonical, race-free pattern for multi-window apps.

## Why this is the right fix

`IsWindowVisible` (and `WebviewWindow::is_visible()`, which on Windows calls `IsWindowVisible` via tao — confirmed in source `tao-0.35.3/src/platform_impl/windows/window.rs:664`) reports whether the window is currently visible considering both `WS_VISIBLE` style AND ancestor window visibility. WebView2 has internal state that can desync from the native Win32 `WS_VISIBLE` flag, causing `IsWindowVisible` to return stale `true` values. We cannot fix that — it's a WebView2 internal.

A manual `AtomicBool` we control is 100% reliable because it reflects exactly the last show/hide we performed, not what Windows/WebView2 think is happening.

## Design

Add two `Arc<AtomicBool>` fields to the existing `HotkeyState` struct in `hotkeys.rs`:

- `write_window_open: Arc<AtomicBool>` — set to `true` when `start-write-flow` listener shows the write window, set to `false` when write window is hidden (via toggle, Esc, X button, or `translate_text` completion)
- `result_window_open: Arc<AtomicBool>` — set to `true` when `emit_result`/`emit_error` shows the result window, set to `false` when `dismiss_result` hides it (or the v0.9.6 OCR toggle hides it)

These flags live inside the `HotkeyState` struct (which is `app.manage()`-ed), so they're accessible from:
- `hotkeys.rs` (the hotkey thread, owns the toggle logic)
- `commands.rs` (owns `emit_result`, `emit_error`, `dismiss_result`)
- `lib.rs` (owns the `start-write-flow` listener)
- `write.js` (frontend — when Esc/X is pressed, notify backend to clear the flag)

For the frontend notification, the existing `hide_window` Tauri command already takes a label, so we extend it to also clear the appropriate flag. This avoids adding a new command.

## Files to Touch

| File | Lines | Change |
|------|-------|--------|
| `src-tauri/src/hotkeys.rs` | 80-92 | Add `write_window_open` and `result_window_open` to `HotkeyState` struct + `new()` |
| `src-tauri/src/hotkeys.rs` | 23-72 | **Remove** the two `is_*_window_visible` helpers (no longer needed) |
| `src-tauri/src/hotkeys.rs` | OCR handler | Replace `is_result_window_visible` with `state.result_window_open.load()` |
| `src-tauri/src/hotkeys.rs` | Write handler | Replace `is_write_window_visible` with `state.write_window_open.load()` |
| `src-tauri/src/commands.rs` | 155-170 (`emit_result`) | Set `result_window_open = true` before `.show()` |
| `src-tauri/src/commands.rs` | 136-151 (`emit_error`) | Set `result_window_open = true` before `.show()` |
| `src-tauri/src/commands.rs` | 1137-1144 (`dismiss_result`) | Set `result_window_open = false` after `.hide()` |
| `src-tauri/src/commands.rs` | 1128-1134 (`hide_window`) | Set `write_window_open = false` when label == "write" |
| `src-tauri/src/lib.rs` | 509-549 (`start-write-flow` listener) | Set `write_window_open = true` before `write_win.show()` |
| `src/result/result.js` | 301-305 | **No change** — Esc still calls `dismiss_result` which clears the flag |
| `src/write/write.js` | 37-41 (`closeWindow`) | **No change** — Esc/X still call `hide_window` which clears the flag |
| `docs/decisions.md` | end | Add ADR-027 |
| `CHANGELOG.md` | top | Add `## [0.9.8]` section |
| 3 version files | version field | Bump `0.9.7` → `0.9.8` |

## Implementation Detail

### Extend `HotkeyState` struct (hotkeys.rs lines 80-92)

Current:
```rust
pub struct HotkeyState {
    shutdown: Arc<AtomicBool>,
    thread_id: Arc<AtomicU32>,
    thread_handle: Option<JoinHandle<()>>,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            thread_id: Arc::new(AtomicU32::new(0)),
            thread_handle: None,
        }
    }
}
```

New:
```rust
pub struct HotkeyState {
    shutdown: Arc<AtomicBool>,
    thread_id: Arc<AtomicU32>,
    thread_handle: Option<JoinHandle<()>>,
    /// Tracks whether the write window is currently shown.
    /// Set by the start-write-flow listener (show) and hide_window/dismiss_write
    /// commands (hide). Used by the Write hotkey toggle to decide whether to
    /// emit start-write-flow or hide the window.
    pub write_window_open: Arc<AtomicBool>,
    /// Tracks whether the result window is currently shown.
    /// Set by emit_result/emit_error (show) and dismiss_result/OCR toggle (hide).
    /// Used by the OCR hotkey toggle to decide whether to emit start-ocr-flow
    /// or hide the window.
    pub result_window_open: Arc<AtomicBool>,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            thread_id: Arc::new(AtomicU32::new(0)),
            thread_handle: None,
            write_window_open: Arc::new(AtomicBool::new(false)),
            result_window_open: Arc::new(AtomicBool::new(false)),
        }
    }
}
```

### Replace visibility helpers in hotkeys.rs (lines 23-72)

**Remove entirely** the two helpers `is_result_window_visible` and `is_write_window_visible`. They used `IsWindowVisible` which is unreliable for our use case. The OCR and Write hotkey handlers will use the `Arc<AtomicBool>` flags directly via a clone of the `Arc`.

### Update hotkey thread closure to clone the flags

The hotkey thread closure (in `register_hotkeys_with_swap`) needs access to the flags. The current code captures `app_handle` and `shutdown` by move. Add the two flag clones:

```rust
let write_open = state.write_window_open.clone();
let result_open = state.result_window_open.clone();
let handle = thread::spawn(move || {
    // ... use write_open, result_open inside the match ...
});
```

### Replace OCR handler toggle logic

Current (lines 270-297 in hotkeys.rs):
```rust
HOTKEY_ID_OCR => {
    app_log!("OCR hotkey pressed!");
    if is_result_window_visible(&app_handle) {
        app_log!("Result window already visible — dismissing via OCR hotkey toggle");
        if let Some(window) = app_handle.get_webview_window("result") {
            let _ = window.hide();
            // ... WS_EX_NOACTIVATE re-apply ...
        }
        return;
    }
    let _ = app_handle.emit("start-ocr-flow", ());
}
```

New:
```rust
HOTKEY_ID_OCR => {
    app_log!("OCR hotkey pressed!");
    if result_open.load(Ordering::SeqCst) {
        app_log!("Result window already visible — dismissing via OCR hotkey toggle");
        if let Some(window) = app_handle.get_webview_window("result") {
            let _ = window.hide();
            result_open.store(false, Ordering::SeqCst);
            // ... WS_EX_NOACTIVATE re-apply ...
        }
        return;
    }
    let _ = app_handle.emit("start-ocr-flow", ());
}
```

### Replace Write handler toggle logic

Current (lines 298-309 in hotkeys.rs):
```rust
HOTKEY_ID_WRITE => {
    app_log!("Write hotkey pressed!");
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

New:
```rust
HOTKEY_ID_WRITE => {
    app_log!("Write hotkey pressed!");
    if write_open.load(Ordering::SeqCst) {
        app_log!("Write window already visible — dismissing via Write hotkey toggle");
        if let Some(window) = app_handle.get_webview_window("write") {
            let _ = window.hide();
            write_open.store(false, Ordering::SeqCst);
        }
        return;
    }
    let _ = app_handle.emit("start-write-flow", ());
}
```

### Set flags in commands.rs

In `emit_result` (line 184) and `emit_error` (line 161), before `result_window.show()`, set `result_window_open = true`. The function needs access to the state — we pass it via the `app_handle.get_state()` pattern, which is how Tauri-managed state is accessed in Tauri 2.

Add at the top of both functions:
```rust
if let Some(state) = app_handle.try_state::<std::sync::Mutex<HotkeyState>>() {
    if let Ok(hk) = state.lock() {
        hk.result_window_open.store(true, Ordering::SeqCst);
    }
}
```

In `dismiss_result` (line 1138), after `window.hide()`, clear the flag:
```rust
window.hide().map_err(|e| e.to_string())?;
if let Some(state) = app_handle.try_state::<std::sync::Mutex<HotkeyState>>() {
    if let Ok(hk) = state.lock() {
        hk.result_window_open.store(false, Ordering::SeqCst);
    }
}
set_result_window_noactivate(&window, true);
```

In `hide_window` (line 1128-1134), clear `write_window_open` if the label is "write":
```rust
pub async fn hide_window(label: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window(&label) {
        window.hide().map_err(|e| e.to_string())?;
        if label == "write" {
            if let Some(state) = app_handle.try_state::<std::sync::Mutex<HotkeyState>>() {
                if let Ok(hk) = state.lock() {
                    hk.write_window_open.store(false, Ordering::SeqCst);
                }
            }
        }
    }
    Ok(())
}
```

### Set flag in lib.rs start-write-flow listener

In the listener at `lib.rs:509-549`, before `write_win.show()` (line 524), set `write_window_open = true`:
```rust
if let Some(write_win) = handle.get_webview_window("write") {
    // Set flag BEFORE show, so the next hotkey press sees it as open
    if let Some(state) = handle.try_state::<std::sync::Mutex<HotkeyState>>() {
        if let Ok(hk) = state.lock() {
            hk.write_window_open.store(true, Ordering::SeqCst);
        }
    }
    let _ = write_win.show();
    let _ = write_win.set_focus();
    // ... positioning ...
}
```

## How it Works (Flow)

### First open (write_window_open = false initially)
1. User presses `Ctrl+Shift+W`
2. `write_open.load()` returns `false`
3. `app_handle.emit("start-write-flow", ())` fires
4. lib.rs listener: sets `write_open = true`, then `write_win.show()`

### Close via toggle (write_window_open = true)
1. User presses `Ctrl+Shift+W` again
2. `write_open.load()` returns `true`
3. `write_win.hide()` + `write_open.store(false)`

### Close via Esc/X in write.js (write_window_open = true)
1. `closeWindow()` calls `invoke('hide_window', { label: 'write' })`
2. `hide_window` does `window.hide()` + sets `write_open = false`

### Re-open after Esc/X (write_window_open = false again)
1. User presses `Ctrl+Shift+W`
2. `write_open.load()` returns `false` (correctly)
3. Emits `start-write-flow` → window reopens

### Close via translate_text completion (write_window_open = true)
1. `translate_text` (commands.rs) calls `write_win.hide()` directly
2. **Bug gap**: this path doesn't clear `write_window_open`!
3. **Fix**: also clear the flag in `translate_text` (commands.rs)

### Close via OCR (write_window_open = true, unusual but possible)
1. User has write window open, then triggers OCR
2. OCR shows result window, but write window stays open (or is hidden? need to check)
3. If write window stays open, no flag change needed (still true)
4. If write window is closed somewhere, need to find that path

Let me check `translate_text` to see exactly what it does with the write window:

Looking at the explore agent's report: "in `translate_text()` (commands.rs línea 714): `write_win.hide()` after emitir el resultado"

So `translate_text` ALSO hides the write window. We need to clear the flag there too. But `translate_text` is a long function; adding the flag-clear at the right place is straightforward.

## How to Wire the Flag into Existing Code

Since the flag lives in `HotkeyState` which is `app.manage()`-ed, every place that needs to read/write the flag uses the pattern:

```rust
if let Some(state) = app_handle.try_state::<std::sync::Mutex<HotkeyState>>() {
    if let Ok(hk) = state.lock() {
        hk.WRITE_FLAG.store(VALUE, Ordering::SeqCst);
    }
}
```

This is the same pattern already used in `save_settings` at `commands.rs:566`:
```rust
let mut hk = hotkey_state.lock().map_err(|e| e.to_string())?;
```

So the pattern is established. The only difference: my flag updates don't need to return errors (they're best-effort), so we use `if let` chains instead of `?`.

## Edge Cases to Verify

- **Race between toggle hide and external hide**: if `hide_window` is called from the frontend (Esc) at almost the same time as the toggle hotkey, both will try to clear the flag. The `store(false)` is idempotent — no harm.
- **State lock contention**: `Mutex<HotkeyState>` is the existing pattern. Brief lock to read/write a single bool is microseconds, negligible.
- **Frontend opens write via UI button** (if exists, not just hotkey): would also need to set the flag. **Out of scope for this change** — the only entry point to show the write window today is the hotkey via `start-write-flow` listener. If a UI button is added in the future, it must also set the flag.
- **App restart**: flags are initialized to `false` on startup, so first open always works.

## Test Plan

User must validate after building:

1. **Build and run** v0.9.8.
2. **Trigger write** (`Ctrl+Shift+W`) → write window appears.
3. **Press `Ctrl+Shift+W` again** → window closes.
4. **Press `Ctrl+Shift+W` a third time** → window reopens (this was the v0.9.7 bug).
5. **Press `Ctrl+Shift+W` a fourth time** → window closes.
6. **Test Esc close**: open write, type, press Esc → window closes, flag clears. Reopen with hotkey → works.
7. **Test X button close**: open write, click X → window closes, flag clears. Reopen with hotkey → works.
8. **Test OCR toggle still works**: trigger OCR, get result, `Ctrl+Shift+T` → closes. Third press → reopens.
9. **Test interaction**: open write, don't close it, trigger OCR → result appears on top. Press `Ctrl+Shift+W` → write closes, result stays. Press `Ctrl+Shift+T` → result closes. Now both flags should be false. Reopen either with hotkey → works.

## Version Bump

- `src-tauri/tauri.conf.json`: `0.9.7` → `0.9.8`
- `src-tauri/Cargo.toml`: `0.9.7` → `0.9.8`
- `package.json`: `0.9.7` → `0.9.8`

## Documentation

- Add to `CHANGELOG.md`:
  ```
  ## [0.9.8] - 2026-06-12
  - fix: Write and OCR hotkey toggles now work consistently across multiple presses by replacing IsWindowVisible with manual state tracking
  ```
- Add to `docs/decisions.md`:
  ```
  ## ADR-027 — Manual state tracking for hotkey toggle visibility
  - **Context**: v0.9.7 toggle only worked once because IsWindowVisible (and equivalently WebviewWindow::is_visible which delegates to it on Windows) returns inconsistent results after a hide() operation on WebView2-backed windows. Confirmed by source: tao-0.35.3/src/platform_impl/windows/window.rs:664 calls util::is_visible which calls IsWindowVisible.
  - **Decision**: Replace IsWindowVisible with manual AtomicBool flags in HotkeyState. Set to true on show, false on hide, from the same code paths that perform the operation.
  - **Why**: 100% reliable, race-free, doesn't depend on Windows/WebView2 internal state. Standard pattern for multi-window apps.
  - **Trade-off**: Every show/hide call site must remember to update the flag. Documented in comments. Mitigated by centralizing the flag updates in 5 well-known locations.
  ```

## Out of Scope (Noted, Not Fixed)

- Frontend-initiated show of write window (e.g. via UI button) would need to set the flag. Currently no such path exists.
- Focus not restored to game on write window close (pre-existing bug).
- Pre-existing warnings (unused imports/variables).
- `translate_text` dead code.
- Double dispatch of `translation-result`.
