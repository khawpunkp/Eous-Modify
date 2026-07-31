mod commands;
mod db;
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
            db::seed::sync_definitions(&mut conn, &app.handle())
                .expect("failed to sync built-in definitions");
            commands::reload::reapply_background_keys_on_startup(&conn);
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
            commands::mods::delete_mod,
            commands::mods::open_mod_folder,
            commands::mods::get_mod_keybinds,
            commands::launcher::launch_game,
            commands::reload::reload_xxmi,
            commands::reload::set_xxmi_background_keys,
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
