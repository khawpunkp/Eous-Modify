//! The one place that flips mods on disk, so every caller gets the same ordering.
//!
//! Enabling and disabling is a folder rename, but three things have to happen around it for a mod to
//! come back the way the user left it — see [`crate::persisted_vars`] for why the state is fragile:
//!
//! 1. **Flush before reading.** The variables the user just changed with an in-game keybind live in
//!    3DMigoto's memory; `d3dx_user.ini` still holds the previous values until a reload. Snapshotting
//!    without flushing first captures stale state.
//! 2. **Snapshot before renaming, restore after.** The keys are derived from the mod's path, so they
//!    only match while the folder carries the name they were written under.
//! 3. **Reload immediately after restoring.** 3DMigoto rewrites `d3dx_user.ini` from its own memory
//!    whenever it saves, so a restored value that hasn't been read back yet can be overwritten. Doing
//!    the reload here rather than from the frontend keeps that window to a single function call
//!    instead of an IPC round trip.
//!
//! Group toggles route through here too. They used to rename members directly, which meant toggling a
//! group discarded every member's in-game choices.

use std::path::Path;

use tauri::State;

use crate::commands::reload;
use crate::mods;
use crate::persisted_vars;
use crate::DbState;

/// A mod that is about to be flipped, resolved before anything touches the disk.
pub struct Flip {
    pub mod_id: i64,
    pub folder_name: String,
    pub was_enabled: bool,
}

impl Flip {
    /// Resolves a mod's current on-disk state by id.
    pub fn resolve(
        conn: &rusqlite::Connection,
        mods_path: &Path,
        mod_id: i64,
    ) -> Result<Self, String> {
        let folder_name: String = conn
            .query_row(
                "SELECT folder_name FROM mods WHERE id = ?1",
                rusqlite::params![mod_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let was_enabled = mods::is_mod_enabled(mods_path, &folder_name).unwrap_or(false);
        Ok(Self { mod_id, folder_name, was_enabled })
    }
}

/// Flips every mod in `flips`, preserving each one's persisted 3DMigoto variables, and reloads once.
///
/// A single flush and a single reload cover the whole batch — a group of eight mods shouldn't mean
/// sixteen keypresses.
pub fn run(state: &State<DbState>, mods_path: &Path, flips: &[Flip]) -> Result<(), String> {
    if flips.is_empty() {
        return Ok(());
    }

    let (auto_reload, game_exe) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        (reload::auto_reload_enabled(&conn), reload::game_executable_from(&conn)?)
    };

    // Only ask for a flush if something is being turned off — that's the direction that loses state,
    // and only if the user opted into us sending keypresses at all.
    let any_disabling = flips.iter().any(|f| f.was_enabled);
    if any_disabling && auto_reload {
        reload::flush_persisted_vars(mods_path, game_exe.as_deref());
    }

    // Snapshot while the folders still carry the names their keys were written under.
    let snapshots: Vec<(i64, Vec<(String, String)>)> = flips
        .iter()
        .filter(|f| f.was_enabled)
        .map(|f| (f.mod_id, persisted_vars::snapshot(mods_path, &f.folder_name)))
        .collect();

    for flip in flips {
        mods::toggle_mod(mods_path, &flip.folder_name)?;
    }

    {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        for (mod_id, vars) in &snapshots {
            persisted_vars::store(&conn, *mod_id, vars)?;
        }
    }

    // Everything being switched on gets written back in one pass, so the file is touched once.
    let to_restore: Vec<(String, String)> = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let mut all = Vec::new();
        for flip in flips.iter().filter(|f| !f.was_enabled) {
            all.extend(persisted_vars::load(&conn, flip.mod_id)?);
        }
        all
    };

    // Never fail the toggle over this: the rename already succeeded and the mod works — it just comes
    // back with default toggles.
    if let Err(e) = persisted_vars::restore(mods_path, &to_restore) {
        eprintln!("[toggle] could not restore persisted variables: {e}");
    }

    if auto_reload {
        if let Err(e) = reload::send_reload(game_exe.as_deref()) {
            eprintln!("[toggle] could not ask XXMI to reload: {e}");
        }
    }

    Ok(())
}
