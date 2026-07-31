use tauri::{AppHandle, State};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::DbState;

/// Spawns the configured game executable directly via `Shell::command()` (the plugin's Rust API),
/// not the plugin's JS-facing scoped `execute` IPC command — confirmed by reading the installed
/// crate's source (`scope.rs`/`lib.rs`): `Shell::command()` calls `Command::new(program)` with no
/// scope validation at all, since that validation only guards the webview-invokable command. Same
/// "bypass the IPC-facing capability scope from Rust code" pattern as `read_image_as_data_url` and
/// the scanner's direct filesystem access — no `shell:allow-execute` capability entry needed, since
/// a static allowlist entry couldn't pre-declare a path the user only chooses at runtime anyway.
#[tauri::command]
pub async fn launch_game(state: State<'_, DbState>, app_handle: AppHandle) -> Result<(), String> {
    let exe_path: String = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT value FROM settings WHERE key = 'game_executable_path'", [], |row| row.get(0))
            .map_err(|_| "Game executable path is not configured. Set it in Settings first.".to_string())?
    };

    let (mut rx, _child) = match app_handle.shell().command(&exe_path).spawn() {
        Ok(pair) => pair,
        Err(e) => {
            let msg = e.to_string();
            return Err(if msg.contains("os error 740") {
                "Failed to launch: the game requires administrator privileges. Try running Eous Modify as administrator."
                    .to_string()
            } else {
                format!("Failed to spawn executable: {}", msg)
            });
        }
    };

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => println!("[game] {}", String::from_utf8_lossy(&line)),
            CommandEvent::Stderr(line) => eprintln!("[game] {}", String::from_utf8_lossy(&line)),
            CommandEvent::Error(e) => eprintln!("[game] error event: {}", e),
            CommandEvent::Terminated(payload) => {
                println!("[game] terminated with code: {:?}", payload.code);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
