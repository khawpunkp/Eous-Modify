mod commands;
mod db;
mod image_format;
mod keybinds;
mod mod_groups;
mod models;
mod mods;
mod scanner;
mod xxmi_config;

use std::sync::Mutex;
use tauri::Manager;

pub struct DbState(pub Mutex<rusqlite::Connection>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init());

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let mut conn = db::init_db(&app_data_dir).expect("failed to initialize database");

            // Before anything reads the reload settings: an install arriving from 0.0.4 has a switch
            // that was turned on under terms that no longer exist, so it starts off rather than
            // silently becoming a UAC prompt on first launch.
            commands::reload::adopt_default_method(&conn);

            // Keep d3dx.ini honest about what the user actually chose. This matters most upgrading
            // from 0.0.4, which wrote check_foreground_window = 0 unconditionally: unless they have
            // since asked for immediate mode, that line is a leftover, and leaving it means their
            // mod keybinds keep firing everywhere. Cheap to reassert every launch, and it repairs
            // the case where a crash left the file disagreeing with the setting.
            if let Ok(mods_folder) = conn.query_row::<String, _, _>(
                "SELECT value FROM settings WHERE key = 'mods_folder_path'",
                [],
                |row| row.get(0),
            ) {
                let wanted = commands::reload::wants_background_hotkeys(&conn);
                let _ = xxmi_config::apply(std::path::Path::new(&mods_folder), wanted);
            }

            // Deferred reload only reaches the game from an elevated process — see
            // `commands::elevation` for why nothing reports the failure otherwise. So when the user
            // has that setting on, restart into an elevated copy now, before the window is ever
            // shown. Declining the prompt is not an error: this copy carries on as it is, and
            // Settings offers the restart again. Immediate mode is deliberately excluded: needing
            // no elevation is the whole reason someone picks it.
            let wants_elevation = commands::reload::auto_reload_enabled(&conn)
                && commands::reload::reload_method(&conn) == commands::reload::ReloadMethod::Deferred
                && !commands::elevation::running_as_admin();

            // Never under `tauri dev`. The elevated copy is not the process cargo launched, so this
            // one exiting reads to the CLI as the app closing, and it takes the dev server down with
            // it — leaving the new window pointed at a devUrl that no longer answers. Said out loud
            // rather than skipped quietly, because a deferred reload that silently does nothing in
            // dev looks like a different bug. `is_dev()` is false in anything `tauri build`
            // produces, so a shipped build still elevates exactly as before.
            if wants_elevation && tauri::is_dev() {
                eprintln!(
                    "[elevation] dev build, staying unelevated; deferred reload will not reach the game"
                );
            } else if wants_elevation {
                match commands::elevation::restart_elevated() {
                    // Abrupt on purpose. Seeding hasn't run and nothing has been written, so there
                    // is nothing to unwind — and the elevated copy is already on its way, which is
                    // not a moment to leave two instances sharing the database.
                    Ok(()) => std::process::exit(0),
                    Err(e) => eprintln!("[elevation] carrying on without administrator: {e}"),
                }
            }

            db::seed::sync_definitions(&mut conn, &app.handle())
                .expect("failed to sync built-in definitions");
            app.manage(DbState(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agents::list_agents,
            commands::agents::get_agent,
            commands::agents::create_agent,
            commands::agents::update_agent,
            commands::agents::delete_agent,
            commands::categories::list_categories,
            commands::elevation::is_elevated,
            commands::elevation::relaunch_as_admin,
            commands::images::read_image_as_data_url,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::open_mods_folder,
            commands::scanner::scan_mods_directory,
            commands::scanner::analyze_archive,
            commands::scanner::import_archive,
            commands::mods::list_mods,
            commands::mods::list_uncategorized_mods,
            commands::mods::toggle_mod_enabled,
            commands::mods::update_mod_info,
            commands::mods::update_mod_category,
            commands::mods::update_mod_files,
            commands::mods::delete_mod,
            commands::mods::open_mod_folder,
            commands::mods::get_mod_keybinds,
            commands::mods::get_mod_preview,
            commands::mods::get_mod_default_preview,
            commands::launcher::launch_game,
            commands::reload::request_reload,
            commands::reload::set_reload_config,
            commands::reload::is_game_running,
            commands::mod_groups::list_mod_groups,
            commands::mod_groups::create_mod_group,
            commands::mod_groups::add_mod_to_group,
            commands::mod_groups::remove_mod_from_group,
            commands::mod_groups::update_mod_group,
            commands::mod_groups::delete_mod_group,
            commands::mod_groups::toggle_mod_group,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
