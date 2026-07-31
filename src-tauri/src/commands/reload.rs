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

/// The game's own process — what 3DMigoto is actually injected into, and therefore what has to be
/// alive for a reload keypress to mean anything.
///
/// Checked *in addition to* whatever executable the user configured, because the Settings path is the
/// thing that **launches** the game and is very often `XXMI Launcher.exe`, which exits as soon as the
/// game is up. Matching only that reports "not running" for a perfectly live game and silently
/// swallows every reload. Safe to hardcode: this app is Zenless Zone Zero only.
#[cfg(windows)]
const GAME_PROCESS_NAMES: &[&str] = &["zenlesszonezero.exe"];

/// Whether the game looks like it's running, or `None` when we genuinely can't tell.
///
/// Used to avoid synthesizing F10 into whatever window happens to be focused when the game isn't even
/// running. `None` means "carry on regardless" rather than "don't", so a detection failure degrades to
/// unconditional behaviour instead of silently disabling reloads.
#[cfg(windows)]
fn game_is_running(exe_path: Option<&str>) -> Option<bool> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut wanted: Vec<String> = GAME_PROCESS_NAMES.iter().map(|n| n.to_string()).collect();
    if let Some(configured) = exe_path
        .and_then(|p| std::path::Path::new(p).file_name())
        .map(|n| n.to_string_lossy().to_lowercase())
    {
        wanted.push(configured);
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    let mut found = false;
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
            if wanted.iter().any(|w| *w == name) {
                found = true;
                break;
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }

    unsafe { CloseHandle(snapshot) }.ok();
    Some(found)
}

/// Sends F10 so XXMI reloads its mods.
///
/// Skipped when we can see the game isn't running: `SendInput` writes to the global input stream, so
/// an F10 sent with the game closed lands in whatever application the user is actually using.
#[allow(unused_variables)]
pub fn send_reload(game_exe: Option<&str>) -> Result<(), String> {
    #[cfg(windows)]
    {
        if game_is_running(game_exe) == Some(false) {
            return Ok(());
        }
        press_f10()
    }

    #[cfg(not(windows))]
    Ok(())
}


/// How long to wait for 3DMigoto to write `d3dx_user.ini` after a reload keypress, and how often to
/// look. Bounded because a game that ignores the keypress must not hang the toggle; polling the
/// modified time rather than sleeping a fixed interval keeps a fast machine fast.
#[cfg(windows)]
const FLUSH_TIMEOUT_MS: u64 = 1_000;
#[cfg(windows)]
const FLUSH_POLL_MS: u64 = 20;

/// Asks 3DMigoto to write its persistent variables to disk, and waits until it has.
///
/// A reload is what makes 3DMigoto flush `d3dx_user.ini`; until then, variables the user changed with
/// an in-game keybind exist only in its memory, and snapshotting the file would capture stale values.
/// Does nothing when the game isn't running — there is nothing to flush, and no point waiting.
#[allow(unused_variables)]
pub fn flush_persisted_vars(mods_folder: &Path, game_exe: Option<&str>) {
    #[cfg(windows)]
    {
        if game_is_running(game_exe) != Some(true) {
            return;
        }

        let Some(user_config) = crate::persisted_vars::user_config_path(mods_folder) else {
            return;
        };
        let modified_before = std::fs::metadata(&user_config).and_then(|m| m.modified()).ok();

        if press_f10().is_err() {
            return;
        }

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(FLUSH_TIMEOUT_MS);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(FLUSH_POLL_MS));
            let modified_now = std::fs::metadata(&user_config).and_then(|m| m.modified()).ok();
            if modified_now.is_some() && modified_now != modified_before {
                return;
            }
        }
    }
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

/// Reads the configured game executable, or `None` when the user hasn't set one yet.
///
/// Note this is the path that *launches* the game, which is frequently `XXMI Launcher.exe` rather than
/// the game itself — see [`GAME_PROCESS_NAMES`] for why that matters.
pub fn game_executable_from(conn: &rusqlite::Connection) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'game_executable_path'",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Whether the user opted into us synthesizing keypresses at all.
pub fn auto_reload_enabled(conn: &rusqlite::Connection) -> bool {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'auto_reload_on_toggle'",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .as_deref()
        == Some("true")
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
