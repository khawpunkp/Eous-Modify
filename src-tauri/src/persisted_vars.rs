//! Carries a mod's in-game toggle choices across a disable/enable cycle.
//!
//! 3DMigoto saves `global persist` variables to `d3dx_user.ini`, keyed by the *path* of the ini that
//! declared them — `$\mods\robot aria\ariahumanzzz.ini\facea2 = 1`. Enabling and disabling renames the
//! mod folder, so that key changes, and on the next config reload 3DMigoto treats the old entry as
//! unrecognised and drops it (`IniHandler.cpp`: "Unknown user settings will be removed from
//! d3dx_user.ini"). Re-enabling restores the folder name but not the values, so every toggle the user
//! set in-game reverts to its `[Constants]` default.
//!
//! So we park them ourselves: snapshot the mod's entries while it is still enabled, then write them
//! back once it is enabled again, before the reload that makes 3DMigoto read the file.
//!
//! Mods declaring an explicit `namespace = …` are already immune — their key contains no folder path —
//! and they simply never match a path-derived prefix, so they're skipped for free.

use std::path::{Path, PathBuf};

/// Where 3DMigoto keeps persisted variables. Configurable via `[Include] user_config` in `d3dx.ini`;
/// we assume the default, which is what XXMI ships.
const USER_CONFIG_FILENAME: &str = "d3dx_user.ini";

/// The section persisted variables live in.
const CONSTANTS_SECTION: &str = "[Constants]";

/// `d3dx_user.ini` sits beside `d3dx.ini`, one level above the Mods folder.
pub fn user_config_path(mods_folder: &Path) -> Option<PathBuf> {
    Some(mods_folder.parent()?.join(USER_CONFIG_FILENAME))
}

/// The key prefix 3DMigoto derives for each of the mod's ini files: the ini path relative to the game
/// directory, lowercased, backslash-separated, leading separator included.
///
/// Must be called while the folder is in the state whose keys you want — the `DISABLED_` prefix is
/// part of the path, so snapshotting has to happen before the rename and restoring after it.
pub fn path_namespaces(mods_folder: &Path, folder_name: &str) -> Vec<String> {
    let Some(game_dir) = mods_folder.parent() else {
        return Vec::new();
    };

    crate::mods::find_mod_ini_paths(mods_folder, folder_name)
        .iter()
        .filter_map(|ini| ini.strip_prefix(game_dir).ok())
        .map(|relative| {
            let joined = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                .collect::<Vec<_>>()
                .join("\\");
            format!("\\{joined}")
        })
        .collect()
}

/// Splits a `d3dx_user.ini` variable line into its key and value, if it is one.
fn split_var_line(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('$') {
        return None;
    }
    let (key, value) = trimmed.split_once('=')?;
    Some((key.trim(), value.trim()))
}

/// True when `key` is a variable declared by an ini under one of `namespaces`.
fn belongs_to(key: &str, namespaces: &[String]) -> bool {
    let lowered = key.to_lowercase();
    namespaces.iter().any(|ns| {
        // `$` + namespace + `\` + variable name. The trailing separator matters: without it
        // `\mods\aria` would also swallow `\mods\aria robot\…`.
        let prefix = format!("${}\\", ns.to_lowercase());
        lowered.starts_with(&prefix)
    })
}

/// Every persisted variable in `contents` belonging to the given namespaces, as `(key, value)`.
pub fn extract(contents: &str, namespaces: &[String]) -> Vec<(String, String)> {
    if namespaces.is_empty() {
        return Vec::new();
    }

    contents
        .lines()
        .filter_map(split_var_line)
        .filter(|(key, _)| belongs_to(key, namespaces))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Writes `vars` into `contents`, replacing any line that already sets the same key and appending the
/// rest to `[Constants]`.
///
/// Line endings and the rest of the file are preserved. 3DMigoto rewrites this file wholesale anyway,
/// but a minimal edit keeps us honest about a file we don't own.
pub fn merge(contents: &str, vars: &[(String, String)]) -> String {
    if vars.is_empty() {
        return contents.to_string();
    }

    let newline = if contents.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out: Vec<String> = Vec::new();
    let mut pending: Vec<&(String, String)> = vars.iter().collect();

    let mut in_constants = false;
    let mut constants_ends_at: Option<usize> = None;

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_constants {
                constants_ends_at = Some(out.len());
            }
            in_constants = trimmed.eq_ignore_ascii_case(CONSTANTS_SECTION);
        }

        // Replace in place so an existing value is updated rather than duplicated.
        if let Some((key, _)) = split_var_line(line) {
            if let Some(index) = pending.iter().position(|(k, _)| k.eq_ignore_ascii_case(key)) {
                let (stored_key, stored_value) = pending.remove(index);
                out.push(format!("{stored_key} = {stored_value}"));
                continue;
            }
        }

        out.push(line.to_string());
    }

    if !pending.is_empty() {
        let new_lines: Vec<String> =
            pending.iter().map(|(key, value)| format!("{key} = {value}")).collect();

        // Land inside [Constants] when we can find it, so 3DMigoto reads them as it expects.
        match constants_ends_at.or(if in_constants { Some(out.len()) } else { None }) {
            Some(at) => {
                for (offset, line) in new_lines.into_iter().enumerate() {
                    out.insert(at + offset, line);
                }
            }
            None => {
                out.push(CONSTANTS_SECTION.to_string());
                out.extend(new_lines);
            }
        }
    }

    let mut result = out.join(newline);
    if contents.ends_with('\n') {
        result.push_str(newline);
    }
    result
}

/// Reads the mod's persisted variables straight off disk. Empty when there's nothing saved yet —
/// a mod with no persistent variables, or a game that has never flushed them.
pub fn snapshot(mods_folder: &Path, folder_name: &str) -> Vec<(String, String)> {
    let Some(path) = user_config_path(mods_folder) else {
        return Vec::new();
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    extract(&contents, &path_namespaces(mods_folder, folder_name))
}

/// Writes previously snapshotted variables back into `d3dx_user.ini`.
///
/// Best-effort by design: this runs during a toggle that has already succeeded on disk, so a missing
/// or unreadable user config must not turn into a failed toggle. The caller logs instead.
pub fn restore(mods_folder: &Path, vars: &[(String, String)]) -> Result<(), String> {
    if vars.is_empty() {
        return Ok(());
    }

    let path = user_config_path(mods_folder)
        .ok_or_else(|| "Could not work out where d3dx_user.ini lives.".to_string())?;

    // 3DMigoto creates this on its first flush; if it isn't there yet there are no live values to
    // preserve alongside ours, so starting from an empty [Constants] is correct.
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = merge(&contents, vars);
    if updated == contents {
        return Ok(());
    }

    std::fs::write(&path, updated).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

/// Parks a mod's variables in the database, replacing whatever was held from a previous cycle.
///
/// An empty snapshot still clears the old rows: the mod genuinely has nothing persisted now, and
/// restoring stale values from two cycles ago would be worse than restoring none.
pub fn store(
    conn: &rusqlite::Connection,
    mod_id: i64,
    vars: &[(String, String)],
) -> Result<(), String> {
    conn.execute("DELETE FROM mod_persisted_vars WHERE mod_id = ?1", rusqlite::params![mod_id])
        .map_err(|e| e.to_string())?;

    for (key, value) in vars {
        conn.execute(
            "INSERT INTO mod_persisted_vars (mod_id, var_key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT(mod_id, var_key) DO UPDATE SET value = excluded.value",
            rusqlite::params![mod_id, key, value],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Everything parked for this mod, as `(key, value)`.
pub fn load(conn: &rusqlite::Connection, mod_id: i64) -> Result<Vec<(String, String)>, String> {
    let mut statement = conn
        .prepare("SELECT var_key, value FROM mod_persisted_vars WHERE mod_id = ?1")
        .map_err(|e| e.to_string())?;

    let rows = statement
        .query_map(rusqlite::params![mod_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_INI: &str = "; AUTOMATICALLY GENERATED FILE - DO NOT EDIT\n\n[Constants]\n$\\zzmiv1\\first_run = 0\n$\\mods\\robot aria\\ariahumanzzz.ini\\wholeb2 = 1\n$\\mods\\robot aria\\ariahumanzzz.ini\\facea2 = 3\n$\\maximiliumm\\belle_summer\\hat = 1\n";

    fn aria_ns() -> Vec<String> {
        vec!["\\mods\\robot aria\\ariahumanzzz.ini".to_string()]
    }

    #[test]
    fn user_config_sits_beside_the_mods_folder() {
        let path = user_config_path(Path::new(r"C:\XXMI\ZZMI\Mods")).unwrap();
        assert_eq!(path, PathBuf::from(r"C:\XXMI\ZZMI\d3dx_user.ini"));
    }

    #[test]
    fn extracts_only_the_mods_own_variables() {
        let vars = extract(USER_INI, &aria_ns());
        assert_eq!(
            vars,
            vec![
                ("$\\mods\\robot aria\\ariahumanzzz.ini\\wholeb2".to_string(), "1".to_string()),
                ("$\\mods\\robot aria\\ariahumanzzz.ini\\facea2".to_string(), "3".to_string()),
            ]
        );
    }

    #[test]
    fn ignores_a_mod_with_its_own_namespace() {
        // Belle Summer keys off `namespace = maximiliumm\belle_summer`, so it never matches a
        // path-derived prefix — it survives renames on its own and needs no snapshot.
        let vars = extract(USER_INI, &vec!["\\mods\\belle summer togglesss".to_string()]);
        assert!(vars.is_empty());
    }

    #[test]
    fn does_not_match_a_folder_that_merely_shares_a_prefix() {
        let vars = extract(USER_INI, &vec!["\\mods\\robot".to_string()]);
        assert!(vars.is_empty(), "\\mods\\robot must not swallow \\mods\\robot aria\\…");
    }

    #[test]
    fn restoring_replaces_an_existing_value_rather_than_duplicating_it() {
        let vars = vec![("$\\mods\\robot aria\\ariahumanzzz.ini\\facea2".to_string(), "7".to_string())];
        let out = merge(USER_INI, &vars);
        assert!(out.contains("facea2 = 7"));
        assert!(!out.contains("facea2 = 3"));
        assert_eq!(out.matches("facea2").count(), 1);
        // Everything else is left alone.
        assert!(out.contains("$\\zzmiv1\\first_run = 0"));
        assert!(out.contains("$\\maximiliumm\\belle_summer\\hat = 1"));
        assert_eq!(out.lines().count(), USER_INI.lines().count());
    }

    #[test]
    fn restoring_adds_a_missing_variable_inside_constants() {
        let without = "; header\n\n[Constants]\n$\\zzmiv1\\first_run = 0\n\n[Present]\nfoo = bar\n";
        let vars = vec![("$\\mods\\robot aria\\ariahumanzzz.ini\\facea2".to_string(), "2".to_string())];
        let out = merge(without, &vars);

        let constants_at = out.find("[Constants]").unwrap();
        let present_at = out.find("[Present]").unwrap();
        let var_at = out.find("facea2").unwrap();
        assert!(constants_at < var_at && var_at < present_at, "must land inside [Constants]:\n{out}");
    }

    #[test]
    fn creates_constants_when_the_file_is_empty() {
        let vars = vec![("$\\mods\\x\\y.ini\\v".to_string(), "1".to_string())];
        let out = merge("", &vars);
        assert!(out.contains("[Constants]"));
        assert!(out.contains("$\\mods\\x\\y.ini\\v = 1"));
    }

    #[test]
    fn snapshot_then_restore_round_trips() {
        let saved = extract(USER_INI, &aria_ns());
        // Simulate 3DMigoto having purged them while the mod was disabled.
        let purged: String = USER_INI
            .lines()
            .filter(|l| !belongs_to(split_var_line(l).map(|(k, _)| k).unwrap_or(""), &aria_ns()))
            .map(|l| format!("{l}\n"))
            .collect();
        assert!(!purged.contains("facea2"));

        let restored = merge(&purged, &saved);
        assert!(restored.contains("$\\mods\\robot aria\\ariahumanzzz.ini\\facea2 = 3"));
        assert!(restored.contains("$\\mods\\robot aria\\ariahumanzzz.ini\\wholeb2 = 1"));
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let crlf = USER_INI.replace('\n', "\r\n");
        let vars = vec![("$\\mods\\robot aria\\ariahumanzzz.ini\\facea2".to_string(), "9".to_string())];
        let out = merge(&crlf, &vars);
        assert_eq!(out.matches('\n').count(), out.matches("\r\n").count());
    }

    #[test]
    fn merging_nothing_leaves_the_file_untouched() {
        assert_eq!(merge(USER_INI, &[]), USER_INI);
    }

    /// TEMPORARY — machine-specific diagnostic, delete after running.
    #[test]
    fn scratch_dump_live_db() {
        let db = std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
            .join("com.eousmodify.modmanager")
            .join("eous-modify.db");
        if !db.exists() {
            eprintln!("skipped: no live db");
            return;
        }

        let conn = rusqlite::Connection::open_with_flags(
            &db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();

        eprintln!("--- settings ---");
        let mut s = conn.prepare("SELECT key, value FROM settings ORDER BY key").unwrap();
        let rows = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap();
        for row in rows {
            let (k, v) = row.unwrap();
            eprintln!("  {k} = {v}");
        }

        let table_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='mod_persisted_vars')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        eprintln!("--- mod_persisted_vars table exists: {table_exists} ---");

        if table_exists {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM mod_persisted_vars", [], |r| r.get(0))
                .unwrap();
            eprintln!("  rows: {count}");
            let mut p = conn
                .prepare(
                    "SELECT v.mod_id, m.name, v.var_key, v.value FROM mod_persisted_vars v
                     LEFT JOIN mods m ON m.id = v.mod_id LIMIT 30",
                )
                .unwrap();
            let rows = p
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })
                .unwrap();
            for row in rows {
                let (id, name, key, value) = row.unwrap();
                eprintln!("  [{id}] {name:?} {key} = {value}");
            }
        }

        eprintln!("--- aria-ish mods in db ---");
        let mut m = conn
            .prepare("SELECT id, name, folder_name FROM mods WHERE lower(name) LIKE '%aria%' OR lower(folder_name) LIKE '%aria%'")
            .unwrap();
        let rows = m
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })
            .unwrap();
        for row in rows {
            let (id, name, folder) = row.unwrap();
            eprintln!("  [{id}] name={name:?} folder={folder:?}");
        }
    }

    fn db_with_one_mod() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO mods (id, name, folder_name) VALUES (1, 'Robot Aria', 'Robot Aria')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn stores_and_loads_a_mods_variables() {
        let conn = db_with_one_mod();
        let vars = vec![
            ("$\\mods\\robot aria\\ariahumanzzz.ini\\facea2".to_string(), "3".to_string()),
            ("$\\mods\\robot aria\\ariahumanzzz.ini\\wholeb2".to_string(), "1".to_string()),
        ];

        store(&conn, 1, &vars).unwrap();
        let mut loaded = load(&conn, 1).unwrap();
        loaded.sort();
        let mut expected = vars.clone();
        expected.sort();
        assert_eq!(loaded, expected);
    }

    #[test]
    fn storing_again_replaces_the_previous_cycles_values() {
        let conn = db_with_one_mod();
        let key = "$\\mods\\robot aria\\ariahumanzzz.ini\\facea2".to_string();

        store(&conn, 1, &[(key.clone(), "3".to_string())]).unwrap();
        store(&conn, 1, &[(key.clone(), "7".to_string())]).unwrap();
        assert_eq!(load(&conn, 1).unwrap(), vec![(key, "7".to_string())]);
    }

    #[test]
    fn storing_an_empty_snapshot_clears_stale_values() {
        // Otherwise a mod that no longer persists anything would be restored from two cycles ago.
        let conn = db_with_one_mod();
        store(&conn, 1, &[("$\\mods\\robot aria\\x.ini\\v".to_string(), "1".to_string())]).unwrap();
        store(&conn, 1, &[]).unwrap();
        assert!(load(&conn, 1).unwrap().is_empty());
    }

    #[test]
    fn deleting_a_mod_takes_its_parked_variables_with_it() {
        let conn = db_with_one_mod();
        store(&conn, 1, &[("$\\mods\\robot aria\\x.ini\\v".to_string(), "1".to_string())]).unwrap();
        conn.execute("DELETE FROM mods WHERE id = 1", []).unwrap();
        assert!(load(&conn, 1).unwrap().is_empty());
    }
}
