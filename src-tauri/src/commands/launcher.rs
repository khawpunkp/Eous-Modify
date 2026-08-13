use rusqlite::OptionalExtension;
use tauri::{AppHandle, State};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::DbState;

/// Windows' ERROR_ELEVATION_REQUIRED. Surfaces as an `io::Error` string from `spawn()`, and can also
/// arrive later as a `CommandEvent::Error`, so both paths below check for it.
const ELEVATION_REQUIRED: &str = "os error 740";

/// XXMI Launcher's own command line: `--nogui` starts the game without showing its window, and
/// `--xxmi <tag>` selects the model importer. `ZZMI` is Zenless Zone Zero's, which is the only game
/// this app supports. Same flags FlairX-Mod-Manager passes for the equivalent feature.
const XXMI_SKIP_LAUNCHER_ARGS: [&str; 3] = ["--nogui", "--xxmi", "ZZMI"];

/// Whether the configured executable is XXMI Launcher rather than a game the user pointed at directly.
///
/// The flags above belong to the launcher. Passing them to the game itself would at best be ignored
/// and at worst confuse its own argument parsing, so the setting only takes effect when the target
/// really is the launcher.
fn is_xxmi_launcher(path: &str) -> bool {
    std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase().contains("xxmi"))
        .unwrap_or(false)
}

/// Re-launches the target through `ShellExecuteW`'s `runas` verb, which raises the UAC prompt. A
/// non-elevated process can't spawn an elevated child any other way on Windows — hence the shell
/// round-trip rather than a plain `Command`. Ported from the pre-rebuild app's
/// `launch_executable_elevated`.
#[cfg(windows)]
fn launch_elevated(path: &str, args: &[&str]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
    }

    let path_wide = wide(path);
    let verb_wide = wide("runas");

    // Kept alive for the duration of the call; ShellExecuteW takes one parameter string, not a list.
    let params = args.join(" ");
    let params_wide = wide(&params);
    let params_ptr = if params.is_empty() { PCWSTR::null() } else { PCWSTR(params_wide.as_ptr()) };

    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR(verb_wide.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            params_ptr,
            None,
            SW_SHOWNORMAL,
        )
    };

    // The returned pseudo-HINSTANCE is really a status: anything above 32 means it launched,
    // anything else is an error code.
    let code = result.0 as isize;
    if code > 32 {
        println!("[game] launched elevated via ShellExecuteW");
        return Ok(());
    }

    if code as i32 == ERROR_CANCELLED.0 as i32 {
        return Err("Launch cancelled — administrator permission was declined.".to_string());
    }
    Err(format!(
        "The game needs administrator privileges, and the elevated launch failed \
         (ShellExecuteW error {}).",
        code
    ))
}

#[cfg(not(windows))]
fn launch_elevated(_path: &str, _args: &[&str]) -> Result<(), String> {
    Err("The game requires administrator privileges, which this platform can't request.".to_string())
}

/// Spawns the configured game executable directly via `Shell::command()` (the plugin's Rust API),
/// not the plugin's JS-facing scoped `execute` IPC command — confirmed by reading the installed
/// crate's source (`scope.rs`/`lib.rs`): `Shell::command()` calls `Command::new(program)` with no
/// scope validation at all, since that validation only guards the webview-invokable command. Same
/// "bypass the IPC-facing capability scope from Rust code" pattern as `read_image_as_data_url` and
/// the scanner's direct filesystem access — no `shell:allow-execute` capability entry needed, since
/// a static allowlist entry couldn't pre-declare a path the user only chooses at runtime anyway.
#[tauri::command]
pub async fn launch_game(state: State<'_, DbState>, app_handle: AppHandle) -> Result<(), String> {
    let (exe_path, skip_launcher) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let exe_path: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'game_executable_path'", [], |row| row.get(0))
            .map_err(|_| "Game executable path is not configured. Set it in Settings first.".to_string())?;
        let skip_launcher: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = 'skip_xxmi_launcher'", [], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        (exe_path, skip_launcher.as_deref() == Some("true"))
    };

    let args: &[&str] = if skip_launcher && is_xxmi_launcher(&exe_path) {
        &XXMI_SKIP_LAUNCHER_ARGS
    } else {
        &[]
    };

    // Try a normal launch first: it needs no UAC prompt, so most games start with no interruption.
    // Only if Windows refuses for lack of elevation do we escalate.
    let (mut rx, _child) = match app_handle.shell().command(&exe_path).args(args).spawn() {
        Ok(pair) => pair,
        Err(e) => {
            let msg = e.to_string();
            return if msg.contains(ELEVATION_REQUIRED) {
                launch_elevated(&exe_path, args)
            } else {
                Err(format!("Failed to spawn executable: {}", msg))
            };
        }
    };

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => println!("[game] {}", String::from_utf8_lossy(&line)),
            CommandEvent::Stderr(line) => eprintln!("[game] {}", String::from_utf8_lossy(&line)),
            CommandEvent::Error(e) => {
                // Elevation can be refused after a nominally successful spawn, so retry here too.
                if e.contains(ELEVATION_REQUIRED) {
                    return launch_elevated(&exe_path, args);
                }
                eprintln!("[game] error event: {}", e);
            }
            CommandEvent::Terminated(payload) => {
                println!("[game] terminated with code: {:?}", payload.code);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
