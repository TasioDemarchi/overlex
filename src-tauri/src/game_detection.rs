// Game Detection module - background thread that monitors the foreground window.
// Runs a dedicated OS thread polling GetForegroundWindow() every 1000ms.
// Emits "game-changed" events when the active window or game context changes.
//
// Windows-only: uses Win32 APIs that are only available on Windows targets.
// Conditionally compiled via #[cfg(windows)] in lib.rs.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::Settings;
use crate::app_log;

use windows::Win32::Foundation::{HWND, RECT, BOOL};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_WIN32,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, GetWindowLongPtrW,
    GetWindowRect, GetSystemMetrics, WS_OVERLAPPEDWINDOW,
    GWL_STYLE, SM_CXSCREEN, SM_CYSCREEN,
};
use windows::Win32::Graphics::Gdi::{
    MonitorFromWindow, GetMonitorInfoW,
    MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::core::PWSTR;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// State managed by Tauri — holds the shutdown signal and thread handle.
pub struct GameDetectorState {
    pub shutdown: Arc<AtomicBool>,
    pub handle: Mutex<Option<JoinHandle<()>>>,
}

// ---------------------------------------------------------------------------
// Payload
// ---------------------------------------------------------------------------

/// Payload emitted on the "game-changed" event.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameChangedPayload {
    pub process_name: Option<String>,
    pub fullscreen_exclusive: bool,
    pub matched_profile: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the leaf filename from a full Windows path (e.g., "example.exe").
fn extract_filename(full_path: &str) -> Option<String> {
    Path::new(full_path)
        .file_name()
        .and_then(|f| f.to_str())
        .map(|s| s.to_string())
}

/// Heuristic: check whether the window is in exclusive fullscreen mode.
///
/// Two checks:
/// 1. The window style lacks WS_OVERLAPPEDWINDOW bits (no border/title bar).
/// 2. The window rect covers at least one full monitor.
fn is_fullscreen_exclusive(hwnd: HWND) -> bool {
    unsafe {
        // 1. Style check — fullscreen exclusive windows have no decorations.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if (style & (WS_OVERLAPPEDWINDOW.0 as isize)) != 0 {
            return false;
        }

        // 2. Rect check — does it cover a full monitor?
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }

        let win_w = rect.right - rect.left;
        let win_h = rect.bottom - rect.top;

        // Primary monitor via GetSystemMetrics
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        if win_w >= screen_w && win_h >= screen_h {
            return true;
        }

        // Multi-monitor: check the monitor this window belongs to.
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };

        if GetMonitorInfoW(monitor, &mut mi).0 != 0 {
            let mon_w = mi.rcMonitor.right - mi.rcMonitor.left;
            let mon_h = mi.rcMonitor.bottom - mi.rcMonitor.top;
            return win_w >= mon_w && win_h >= mon_h;
        }

        false
    }
}

// ---------------------------------------------------------------------------
// Core: spawn_detector
// ---------------------------------------------------------------------------

/// Spin up the game-detection background thread.
///
/// Returns the `JoinHandle` so the caller can store it for graceful shutdown.
pub fn spawn_detector(
    app_handle: AppHandle,
    shutdown: Arc<AtomicBool>,
    settings: Arc<Mutex<Settings>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut last_hwnd: Option<isize> = None;
        let mut last_process_name: Option<String> = None;

        loop {
            if shutdown.load(Ordering::SeqCst) {
                app_log!("[GAME_DETECT] Shutdown signal received, exiting thread");
                break;
            }

            let hwnd = unsafe { GetForegroundWindow() };

            // ---------------------------------------------------------------
            // No foreground window
            // ---------------------------------------------------------------
            if hwnd.0.is_null() {
                let payload = GameChangedPayload {
                    process_name: None,
                    fullscreen_exclusive: false,
                    matched_profile: None,
                };
                let _ = app_handle.emit("game-changed", &payload);
                last_hwnd = None;
                last_process_name = None;
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            let hwnd_raw = hwnd.0 as isize;

            // ---------------------------------------------------------------
            // Same window — nothing changed
            // ---------------------------------------------------------------
            if last_hwnd == Some(hwnd_raw) {
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            // ---------------------------------------------------------------
            // Resolve process name
            // ---------------------------------------------------------------
            let mut pid: u32 = 0;
            let _tid = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };

            if pid == 0 {
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            let process_name: Option<String> = match unsafe {
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL(0), pid)
            } {
                Ok(handle) => {
                    let mut buffer = vec![0u16; 260];
                    let mut size = buffer.len() as u32;
                    let result = unsafe {
                        QueryFullProcessImageNameW(
                            handle,
                            PROCESS_NAME_WIN32,
                            PWSTR::from_raw(buffer.as_mut_ptr()),
                            &mut size,
                        )
                    };
                    // Always close the handle regardless of query outcome.
                    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };

                    match result {
                        Ok(()) => {
                            let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
                            let file_name = extract_filename(&full_path);
                            if let Some(ref name) = file_name {
                                // Skip our own process — OverLex windows (Settings, etc.)
                                // should not trigger game-changed events.
                                if name.eq_ignore_ascii_case("overlex.exe") {
                                    app_log!("[GAME_DETECT] Skipping own process: {} (PID {pid})", name);
                                    last_hwnd = Some(hwnd_raw);
                                    thread::sleep(Duration::from_millis(1000));
                                    continue;
                                }
                                app_log!(
                                    "[GAME_DETECT] Foreground: {} (PID {pid})",
                                    name
                                );
                            }
                            file_name
                        }
                        Err(_) => {
                            app_log!("[GAME_DETECT] QueryFullProcessImageNameW failed for PID {pid}");
                            None
                        }
                    }
                }
                Err(_) => {
                    app_log!(
                        "[GAME_DETECT] OpenProcess denied for PID {pid} — likely a protected process"
                    );
                    // Emit with no process info (keep fullscreen check in case it's a game).
                    let fullscreen = is_fullscreen_exclusive(hwnd);
                    let payload = GameChangedPayload {
                        process_name: None,
                        fullscreen_exclusive: fullscreen,
                        matched_profile: None,
                    };
                    let _ = app_handle.emit("game-changed", &payload);
                    last_hwnd = Some(hwnd_raw);
                    last_process_name = None;
                    thread::sleep(Duration::from_millis(1000));
                    continue;
                }
            };

            // ---------------------------------------------------------------
            // Fullscreen check
            // ---------------------------------------------------------------
            let fullscreen = is_fullscreen_exclusive(hwnd);

            // ---------------------------------------------------------------
            // Profile matching (case-insensitive)
            // ---------------------------------------------------------------
            let matched_profile = process_name.as_ref().and_then(|name| {
                let settings_guard = settings.lock().unwrap();
                let profiles = settings_guard.profiles.clone();
                drop(settings_guard);

                profiles.iter().find_map(|profile| {
                    if profile
                        .process_names
                        .iter()
                        .any(|p| p.to_lowercase() == name.to_lowercase())
                    {
                        Some(profile.display_name.clone())
                    } else {
                        None
                    }
                })
            });

            // ---------------------------------------------------------------
            // Dedup: if only the HWND changed but the process is the same,
            //         skip the event (different windows of the same game).
            // ---------------------------------------------------------------
            if process_name.is_some()
                && last_process_name.as_deref() == process_name.as_deref()
            {
                last_hwnd = Some(hwnd_raw);
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            // ---------------------------------------------------------------
            // Emit
            // ---------------------------------------------------------------
            let payload = GameChangedPayload {
                process_name: process_name.clone(),
                fullscreen_exclusive: fullscreen,
                matched_profile,
            };
            app_log!("[GAME_DETECT] Emitting game-changed: {:?}", &payload);
            let _ = app_handle.emit("game-changed", &payload);

            last_hwnd = Some(hwnd_raw);
            last_process_name = process_name;

            thread::sleep(Duration::from_millis(1000));
        }

        app_log!("[GAME_DETECT] Detector thread exited");
    })
}
