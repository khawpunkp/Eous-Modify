//! Asks XXMI/3DMigoto to reload its mods, without weakening its hotkey handling to do it.
//!
//! 3DMigoto reloads on F10, and there is no IPC to ask it directly, so a synthetic keypress is the
//! only lever. But it ignores hotkeys unless its own game window is focused — which it never is at
//! the moment a mod is toggled, because the user is looking at this app.
//!
//! The previous approach set `check_foreground_window = 0` in `d3dx.ini` so the keypress would land
//! anyway. That works, and it is what FlairX-Mod-Manager does, but the setting is not scoped to our
//! keypress: it makes 3DMigoto act on *every* hotkey system-wide. Mods routinely bind bare letters and
//! digits, so typing in a chat window silently toggles them. One automatic F10 is not worth that.
//!
//! So the reload is deferred instead. A toggle records that one is due; when the game window next
//! comes to the foreground, the keypress goes out and lands under 3DMigoto's own default rules. You
//! alt-tab back and the mods are already reloaded, which is the first moment you could have noticed
//! either way.

use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::OptionalExtension;
use tauri::State;

use crate::DbState;

/// A reload is owed to the game.
static RELOAD_PENDING: AtomicBool = AtomicBool::new(false);
/// A watcher thread is already running, so further toggles just re-arm the flag above.
static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

/// How often to ask which window is in front, and how long to keep asking.
///
/// Half a second is imperceptible against alt-tabbing, and the loop only runs while something is
/// actually owed. Giving up after ten minutes stops a thread spinning forever when the game was
/// closed before the reload could be delivered.
#[cfg(windows)]
const POLL_INTERVAL_MS: u64 = 500;
#[cfg(windows)]
const GIVE_UP_AFTER_SECS: u64 = 600;

/// Milliseconds to hold F10 down. 3DMigoto can miss an instantaneous press, so this matches the
/// 50ms hold No-Reload-Mod-Manager settled on.
#[cfg(windows)]
const KEY_HOLD_MS: u64 = 50;

/// The game's own process — what 3DMigoto is injected into.
///
/// Checked alongside whatever executable the user configured, because the Settings path is the thing
/// that *launches* the game and is very often `XXMI Launcher.exe`, which exits as soon as the game is
/// up. Safe to hardcode: this app is Zenless Zone Zero only.
#[cfg(windows)]
const GAME_PROCESS_NAMES: &[&str] = &["zenlesszonezero.exe"];

#[cfg(windows)]
fn press_f10() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VK_F10,
    };

    fn event(flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_F10,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    let size = std::mem::size_of::<INPUT>() as i32;
    let sent = unsafe { SendInput(&[event(KEYBD_EVENT_FLAGS(0))], size) };
    if sent == 0 {
        return Err("Windows rejected the F10 keypress.".to_string());
    }

    std::thread::sleep(std::time::Duration::from_millis(KEY_HOLD_MS));
    unsafe { SendInput(&[event(KEYEVENTF_KEYUP)], size) };
    Ok(())
}

/// The executable name of whichever window is currently in front, lowercased.
#[cfg(windows)]
fn foreground_process_name() -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut buffer = [0u16; 260];
    let mut length = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    unsafe { CloseHandle(handle) }.ok();
    queried.ok()?;

    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    std::path::Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
}

/// Whether the window in front belongs to the game.
#[cfg(windows)]
fn game_is_focused(configured_exe: Option<&str>) -> bool {
    let Some(current) = foreground_process_name() else {
        return false;
    };

    if GAME_PROCESS_NAMES.contains(&current.as_str()) {
        return true;
    }

    configured_exe
        .and_then(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().to_lowercase() == current)
        .unwrap_or(false)
}

/// Reads the configured game executable, or `None` when the user hasn't set one yet.
fn game_executable(conn: &rusqlite::Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'game_executable_path'",
        [],
        |row| row.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Notes that the game owes a reload, and starts watching for its window if nothing is already.
///
/// Returns immediately — the keypress happens later, whenever the game is next in front. Repeated
/// toggles collapse into the single pending reload they all want.
#[tauri::command]
pub fn request_reload(state: State<DbState>) -> Result<(), String> {
    let configured_exe = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        game_executable(&conn)
    };

    RELOAD_PENDING.store(true, Ordering::SeqCst);

    #[cfg(windows)]
    {
        if WATCHER_RUNNING.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        std::thread::spawn(move || {
            let deadline =
                std::time::Instant::now() + std::time::Duration::from_secs(GIVE_UP_AFTER_SECS);

            while RELOAD_PENDING.load(Ordering::SeqCst) {
                if std::time::Instant::now() > deadline {
                    RELOAD_PENDING.store(false, Ordering::SeqCst);
                    break;
                }

                if game_is_focused(configured_exe.as_deref()) {
                    if let Err(e) = press_f10() {
                        eprintln!("[reload] could not send F10: {e}");
                    }
                    RELOAD_PENDING.store(false, Ordering::SeqCst);
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
            }

            WATCHER_RUNNING.store(false, Ordering::SeqCst);
        });
    }

    #[cfg(not(windows))]
    {
        let _ = configured_exe;
        RELOAD_PENDING.store(false, Ordering::SeqCst);
    }

    Ok(())
}
