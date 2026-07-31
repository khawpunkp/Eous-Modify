//! Asks XXMI/3DMigoto to reload its mods by synthesizing the F10 keypress it already listens for.
//!
//! There is no IPC to talk to 3DMigoto, so a global synthetic keypress is the only lever — the same
//! approach No-Reload-Mod-Manager uses. `SendInput` delivers to whatever window currently has focus,
//! which means this only reaches the game while the game is focused; see `foreground_is_ours`.

/// Milliseconds to hold F10 down. 3DMigoto can miss an instantaneous press, so this matches the
/// 50ms hold No-Reload-Mod-Manager settled on.
#[cfg(windows)]
const KEY_HOLD_MS: u64 = 50;

/// True when the focused window belongs to this process. `SendInput` goes to the foreground window,
/// so pressing F10 while our own window is focused would just deliver it to ourselves — silently
/// doing nothing. Checking by PID (rather than a specific HWND) covers every window we own.
#[cfg(windows)]
fn foreground_is_ours() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return false;
    }

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(foreground, Some(&mut pid)) };
    pid == std::process::id()
}

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

/// Sends F10 so XXMI reloads its mods. Returns `false` (rather than erroring) when the keypress was
/// deliberately skipped because our own window had focus — the caller treats that as "not now",
/// since it's the normal case while the user is working inside the app.
#[tauri::command]
pub fn reload_xxmi() -> Result<bool, String> {
    #[cfg(windows)]
    {
        if foreground_is_ours() {
            return Ok(false);
        }
        press_f10()?;
        Ok(true)
    }

    #[cfg(not(windows))]
    Ok(false)
}
