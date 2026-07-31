//! Asks XXMI/3DMigoto to reload its mods by synthesizing the F10 keypress it already listens for.
//!
//! There is no IPC to talk to 3DMigoto, so a synthetic keypress is the only lever — the same approach
//! No-Reload-Mod-Manager uses. `SendInput` injects into Windows' global input stream, which 3DMigoto
//! polls, so the keypress reaches it regardless of which window is focused. What decides whether
//! 3DMigoto *acts* on it is `check_foreground_window` in `d3dx.ini`; see [`crate::xxmi_config`].

use std::path::Path;

use rusqlite::OptionalExtension;
use tauri::State;

use crate::DbState;

/// Milliseconds to hold F10 down. 3DMigoto can miss an instantaneous press, so this matches the
/// 50ms hold No-Reload-Mod-Manager settled on.
#[cfg(windows)]
const KEY_HOLD_MS: u64 = 50;

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

/// Sends F10 so XXMI reloads its mods.
#[tauri::command]
pub fn reload_xxmi() -> Result<(), String> {
    #[cfg(windows)]
    {
        press_f10()
    }

    #[cfg(not(windows))]
    Ok(())
}

/// Reads the configured Mods folder, or `None` when the user hasn't set one yet.
fn mods_folder(state: &State<DbState>) -> Result<Option<String>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'mods_folder_path'",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Turns 3DMigoto's background-hotkey handling on or off, so a reload sent while this app is the
/// focused window actually lands. Called when the auto-reload switch changes.
#[tauri::command]
pub fn set_xxmi_background_keys(enabled: bool, state: State<DbState>) -> Result<(), String> {
    let folder = mods_folder(&state)?
        .ok_or_else(|| "Set your Mods Folder first — that's how we find d3dx.ini.".to_string())?;

    crate::xxmi_config::apply(Path::new(&folder), enabled)
}

/// Re-applies the setting at startup when auto-reload is on.
///
/// XXMI Launcher replaces `d3dx.ini` when it updates the game's mod loader (it backs the old one up
/// first, which is why installs accumulate `Backups\ZZMI <date>\` folders), so a setting written once
/// silently reverts and the feature stops working with no visible cause.
pub fn reapply_background_keys_on_startup(conn: &rusqlite::Connection) {
    let enabled: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'auto_reload_on_toggle'",
            [],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();

    if enabled.as_deref() != Some("true") {
        return;
    }

    let folder: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'mods_folder_path'",
            [],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();

    // Best-effort: a missing folder or an unreadable d3dx.ini must never stop the app from starting.
    // The user sees the real error if they toggle the switch.
    if let Some(folder) = folder {
        if let Err(e) = crate::xxmi_config::apply(Path::new(&folder), true) {
            eprintln!("[reload] could not re-apply background hotkeys: {e}");
        }
    }
}
