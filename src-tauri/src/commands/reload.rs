//! Asks XXMI/3DMigoto to reload its mods, without weakening its hotkey handling to do it.
//!
//! 3DMigoto reloads on F10, and there is no IPC to ask it directly, so a synthetic keypress is the
//! only lever. But it ignores hotkeys unless its own game window is focused — which it never is at
//! the moment a mod is toggled, because the user is looking at this app.
//!
//! There are two ways round that, they cost different things, and neither is free — so the user
//! picks. See [`ReloadMethod`].
//!
//! **Deferred** waits. A toggle records that a reload is due; when the game window next comes to the
//! foreground, the keypress goes out under 3DMigoto's own shipped rules, and `d3dx.ini` is never
//! touched. You alt-tab back and the mods have already reloaded, which is the first moment you could
//! have noticed either way. The catch is elevation: Zenless Zone Zero runs as administrator, and
//! Windows' UIPI drops synthetic input crossing from a lower-integrity process into a higher one.
//! `SendInput` returns success regardless, so this whole path runs and reports fine while the game
//! never sees a thing — which is exactly what it did for three releases. `commands::elevation` is
//! what keeps that from happening silently.
//!
//! **Immediate** sets `check_foreground_window = 0` so 3DMigoto acts on the F10 while *our* window
//! is in front, the way FlairX-Mod-Manager does it. No elevation needed, because the injected input
//! never has to reach the elevated game — it lands in a window we own and 3DMigoto reads global key
//! state. The catch is that the setting is not scoped to our keypress: 3DMigoto now acts on every
//! hotkey system-wide, and mods routinely bind bare letters and digits, so typing in a chat window
//! silently toggles them.

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
/// This is dead time the user sits through after alt-tabbing, so it is kept short: a process
/// snapshot is cheap, and the loop only runs at all while a reload is actually owed. Giving up after
/// ten minutes stops a thread spinning forever when the game was closed before it could be
/// delivered.
#[cfg(windows)]
const POLL_INTERVAL_MS: u64 = 50;
#[cfg(windows)]
const GIVE_UP_AFTER_SECS: u64 = 600;

/// Milliseconds to hold F10 down. 3DMigoto can miss an instantaneous press, so this matches the
/// 50ms hold No-Reload-Mod-Manager settled on.
#[cfg(windows)]
const KEY_HOLD_MS: u64 = 50;

/// How long to let the game settle after its window comes forward, before pressing anything.
///
/// Not zero, and not tuned to the last millisecond either. 3DMigoto samples key state per rendered
/// frame, and a window that has only just come forward may not be presenting yet — a press that
/// lands in that gap is simply lost, and nothing reports it, which is the exact failure mode that
/// cost this feature three releases. So the number buys insurance against a silent miss, and is
/// only worth shaving while reloads still land every time.
#[cfg(windows)]
const SETTLE_AFTER_FOCUS_MS: u64 = 100;

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

    use windows::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo;

    // dwExtraInfo carries GetMessageExtraInfo() rather than a bare 0, matching
    // No-Reload-Mod-Manager — the one other tool known to drive 3DMigoto this way on this game.
    // Not what made the reload work (elevation was), but there is no reason to differ from the
    // implementation with the track record.
    let extra_info = unsafe { GetMessageExtraInfo() }.0 as usize;

    let event = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F10,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    };

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
///
/// Resolved through a ToolHelp snapshot rather than by opening the process. `QueryFullProcessImageNameW`
/// is the obvious way to do this and it does not work here: Zenless Zone Zero is a protected process, so
/// opening a handle to it yields nothing even for `PROCESS_QUERY_LIMITED_INFORMATION` — the same reason
/// Windows itself shows a blank path for it. A snapshot reports names without ever opening anything.
#[cfg(windows)]
fn foreground_process_name() -> Option<String> {
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

    process_name_for_pid(pid)
}

/// Looks a pid up in a process snapshot, returning its executable name lowercased.
#[cfg(windows)]
fn process_name_for_pid(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    let mut found = None;
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            if entry.th32ProcessID == pid {
                let len =
                    entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                found = Some(String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase());
                break;
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }

    unsafe { CloseHandle(snapshot) }.ok();
    found
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

/// Whether the game is running at all — not merely focused.
///
/// Lives here rather than in `launcher` because the knowledge of what the game's process is called
/// already does. Enumerating is necessary: the foreground check above cannot answer this, since the
/// case that matters is precisely the game running behind another window.
#[tauri::command]
pub fn is_game_running(state: State<DbState>) -> Result<bool, String> {
    let configured_exe = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        game_executable(&conn)
    };

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };

        let mut wanted: Vec<String> = GAME_PROCESS_NAMES.iter().map(|n| n.to_string()).collect();
        if let Some(name) = configured_exe
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_lowercase())
        {
            // The launcher counts too: it is running only while it is on screen, which is still a
            // moment when starting the game again would be wrong.
            wanted.push(name);
        }

        let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return Ok(false);
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = false;
        if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
            loop {
                let len =
                    entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
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
        Ok(found)
    }

    #[cfg(not(windows))]
    {
        let _ = configured_exe;
        Ok(false)
    }
}

/// Settings-table key for the auto-reload switch. Mirrors the frontend's `AUTO_RELOAD_KEY`; the two
/// have to agree, since the frontend writes it and startup reads it.
const AUTO_RELOAD_KEY: &str = "auto_reload_on_toggle";

/// Settings-table key for which of the two deliveries below the user picked.
const RELOAD_METHOD_KEY: &str = "reload_method";

/// How the F10 gets to 3DMigoto. The two differ in what they cost the user, not in what they do —
/// see `crate::xxmi_config` for the full comparison.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReloadMethod {
    /// Wait for the game window. Needs Eous elevated; leaves `d3dx.ini` alone.
    Deferred,
    /// Press F10 now, with `check_foreground_window = 0` making 3DMigoto accept it from any window.
    /// Needs no elevation; hands every mod keybind to whatever you are typing in.
    Immediate,
}

/// Defaults to [`ReloadMethod::Deferred`] for anything unrecognised, including the missing row on a
/// fresh install. The safe one is the one you land on by accident.
pub fn reload_method(conn: &rusqlite::Connection) -> ReloadMethod {
    let stored: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [RELOAD_METHOD_KEY], |row| row.get(0))
        .optional()
        .ok()
        .flatten();

    match stored.as_deref() {
        Some("immediate") => ReloadMethod::Immediate,
        _ => ReloadMethod::Deferred,
    }
}

/// Switches the feature off once, for anyone arriving from a version that had no choice to make.
///
/// A missing `reload_method` row means this install predates the two options — nobody who has seen
/// the new Settings can be missing it, since picking either writes it. If such an install had the
/// switch on, it was turned on under 0.0.4's terms: instant reloads, no administrator, and the
/// keybind leak nobody was told about. Every one of those terms has changed, and the nearest
/// replacement, deferred delivery, would raise a UAC prompt on first launch that the user never
/// agreed to — the exact thing that makes people distrust an update.
///
/// So it starts off. One click to turn back on, with both options and their costs visible, which is
/// a better trade than a permission prompt arriving unannounced.
pub fn adopt_default_method(conn: &rusqlite::Connection) {
    let already_chosen: Option<i64> = conn
        .query_row("SELECT 1 FROM settings WHERE key = ?1", [RELOAD_METHOD_KEY], |row| row.get(0))
        .optional()
        .ok()
        .flatten();

    if already_chosen.is_some() {
        return;
    }

    let store = |key: &str, value: &str| {
        let _ = conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        );
    };

    // Stamped even on a fresh install, where it changes nothing — it is what stops this running a
    // second time and overriding a choice the user has since made.
    store(RELOAD_METHOD_KEY, "deferred");

    if auto_reload_enabled(conn) {
        store(AUTO_RELOAD_KEY, "false");
    }
}

/// Whether the user's choice means `d3dx.ini` should currently carry our line.
///
/// Only ever true for immediate mode with the switch on. Everything else — switched off, deferred,
/// fresh install — wants that file back the way XXMI shipped it.
pub fn wants_background_hotkeys(conn: &rusqlite::Connection) -> bool {
    auto_reload_enabled(conn) && reload_method(conn) == ReloadMethod::Immediate
}

/// Reads the configured Mods folder, or `None` when the user hasn't set one yet.
fn mods_folder(conn: &rusqlite::Connection) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = 'mods_folder_path'", [], |row| row.get(0))
        .optional()
        .ok()
        .flatten()
}

/// Stores the switch and the method together, then brings `d3dx.ini` into line with them.
///
/// One entry point for both controls on purpose: the file edit depends on the pair, so letting the
/// frontend write them separately would mean a moment where the stored state and the file disagree,
/// and whichever setting was written second would decide the outcome.
#[tauri::command]
pub fn set_reload_config(
    state: State<DbState>,
    enabled: bool,
    method: String,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    for (key, value) in [(AUTO_RELOAD_KEY, enabled.to_string()), (RELOAD_METHOD_KEY, method)] {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )
        .map_err(|e| e.to_string())?;
    }

    let wanted = wants_background_hotkeys(&conn);
    let Some(folder) = mods_folder(&conn) else {
        // Nothing to write to yet. Only worth complaining about if we were about to turn the file
        // edit on — reverting a file we can't find is already the state we wanted.
        return if wanted {
            Err("Set your Mods folder in Settings first — that's how Eous finds d3dx.ini."
                .to_string())
        } else {
            Ok(())
        };
    };

    crate::xxmi_config::apply(std::path::Path::new(&folder), wanted)
}

/// Whether the user asked for reloads on toggle. Off unless explicitly enabled — including on a fresh
/// install, where the row does not exist at all, so nothing prompts for administrator uninvited.
pub fn auto_reload_enabled(conn: &rusqlite::Connection) -> bool {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [AUTO_RELOAD_KEY], |row| {
        row.get::<_, String>(0)
    })
    .optional()
    .ok()
    .flatten()
    .as_deref()
        == Some("true")
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
    let (configured_exe, method) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        (game_executable(&conn), reload_method(&conn))
    };

    // Immediate mode has nothing to wait for. 3DMigoto is accepting hotkeys from every window, so
    // the keypress lands from the one the user is looking at — which is also why this needs no
    // elevation: the injected input never has to reach the elevated game to be seen.
    if method == ReloadMethod::Immediate {
        #[cfg(windows)]
        {
            return press_f10();
        }
        #[cfg(not(windows))]
        {
            let _ = configured_exe;
            return Ok(());
        }
    }

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
                    // Let the game start drawing again before pressing anything. 3DMigoto samples
                    // key state once per rendered frame, and a window that has only just come
                    // forward is still mid-transition, so a press delivered this instant may land
                    // between frames. Cheap insurance against a race nobody wants to debug twice,
                    // and imperceptible next to the alt-tab it follows.
                    std::thread::sleep(std::time::Duration::from_millis(SETTLE_AFTER_FOCUS_MS));

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
