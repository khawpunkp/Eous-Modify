pub mod archive;
pub mod deduce;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::Serialize;
use walkdir::WalkDir;

use deduce::{
    deduce_mod_info, fetch_deduction_maps, find_category_match, has_ini_file, DeductionMaps,
    DISABLED_PREFIX,
};

use crate::mods::update_mod_category;
use crate::scanner::archive::{AGENTS_SUBDIR, MISC_SUBDIR};
use crate::scanner::deduce::agent_slug_exists;

/// Top-level folders that organise the library rather than being a mod.
///
/// The first four are the layout this app writes. The rest are folders earlier versions wrote and
/// that still exist in installed libraries: four retired categories and the old name for misc. They
/// stay listed because each one holds many mods, and mistaking such a folder for a single mod would
/// give every mod inside it one identity.
const STRUCTURAL_TOP_LEVEL: &[&str] = &[
    AGENTS_SUBDIR,
    MISC_SUBDIR,
    "ui",
    "bangboos",
    "npcs",
    "enemies",
    "weapons",
    "objects",
    "_uncategorized",
];

/// Whether `relative` is a folder that exists to organise mods, not to be one.
fn is_structural_folder(relative: &Path, maps: &DeductionMaps) -> bool {
    let parts: Vec<String> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();

    match parts.len() {
        // agents/<slug> — the per-agent level, always structural.
        2 if parts[0] == AGENTS_SUBDIR => true,
        1 => {
            let name = &parts[0];
            STRUCTURAL_TOP_LEVEL.contains(&name.as_str())
                // A category folder, whatever the definitions currently call it.
                || find_category_match(name, maps).is_some()
                // The pre-agents/ layout put each agent's folder at the top level. Those are still
                // on disk until a scan moves them, and each holds every mod for that agent.
                || agent_slug_exists(name, maps)
        }
        _ => false,
    }
}

/// Whether a folder holds exactly one thing and nothing of its own — the shape of a wrapper that
/// exists only to carry the mod inside it, such as an extracted archive folder.
///
/// Both halves matter. Without "exactly one", climbing up would gather a folder's several mods into
/// one. Without "no .ini of its own", it would swallow a mod that has a nested variant beneath it.
fn is_single_mod_wrapper(dir: &Path) -> bool {
    if has_ini_file(dir) {
        return false;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    let mut directories = 0usize;
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            directories += 1;
            if directories > 1 {
                return false;
            }
        }
    }
    directories == 1
}

/// The folder that should be treated as the mod, given the folder an `.ini` was found in.
///
/// A downloaded mod usually arrives as a named folder wrapping a generically named one — "Astra
/// Shining Eridu" holding "skin01". The `.ini` is in the inner folder, so that is where detection
/// starts, but the outer folder is the one carrying the name that means anything to a person. Taking
/// the inner one leaves a mod called "skin01" and throws the real name away, on disk and in the UI.
///
/// So it climbs, stopping at anything structural and at any folder holding more than the one mod.
fn resolve_mod_root(ini_folder: PathBuf, base_mods_path: &Path, maps: &DeductionMaps) -> PathBuf {
    let mut root = ini_folder;

    while let Some(parent) = root.parent().map(|p| p.to_path_buf()) {
        if parent == base_mods_path {
            break;
        }
        let Ok(relative) = parent.strip_prefix(base_mods_path) else {
            break;
        };
        if is_structural_folder(relative, maps) || !is_single_mod_wrapper(&parent) {
            break;
        }
        root = parent;
    }

    root
}

/// Ports the old app's `scan_mods_directory`: walks the mods folder, identifies mod folders by
/// a non-excluded `.ini` file, fixes up a `DISABLED` -> `DISABLED_` naming inconsistency, runs the
/// deduction pipeline for new folders, re-checks already-known mods that still have no agent in
/// case they can now be matched, then prunes DB rows for mods no longer found on disk.
///
/// `on_progress(processed_count, current_path)` fires once per mod folder found — kept as a plain
/// closure (no Tauri `AppHandle`) so this can run in a unit test as well as behind a real command.
/// One mod folder as the walk found it: where it is now, and the name the database keys it by.
struct FoundMod {
    /// Absolute path to the folder that *is* the mod, after `resolve_mod_root`.
    path: PathBuf,
    /// Path relative to the mods folder with any `DISABLED_` prefix stripped — the `folder_name`
    /// column's form, so an enabled and a disabled mod are the same row either way.
    folder_name: String,
}

struct WalkOutcome {
    mods: Vec<FoundMod>,
    renamed: usize,
    errors: usize,
}

/// Finds every mod folder, and decides which folder *is* the mod.
///
/// Shared by the real scan and the dry run on purpose. A dry run that worked this out separately
/// could disagree with the scan it is supposed to be previewing, which would make it worse than
/// having none — so there is one implementation and `apply_renames` is the only thing that differs.
fn walk_mod_folders(
    base_mods_path: &Path,
    maps: &DeductionMaps,
    apply_renames: bool,
    mut on_progress: impl FnMut(usize, &str),
) -> WalkOutcome {
    let mut mods = Vec::new();
    let mut renamed = 0usize;
    let mut errors = 0usize;
    let mut processed = 0usize;

    let mut walker = WalkDir::new(base_mods_path).min_depth(1).into_iter();

    while let Some(entry_result) = walker.next() {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[scan] error accessing entry: {}", e);
                errors += 1;
                continue;
            }
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        let mut current_path = entry.path().to_path_buf();
        let filename = current_path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Fix up a `DISABLED` (missing underscore) prefix before classifying the folder. A dry run
        // reports what it would do without doing it, so it works from the name already on disk.
        if filename.starts_with("DISABLED") && !filename.starts_with(DISABLED_PREFIX) {
            let new_filename = format!("{}{}", DISABLED_PREFIX, filename.strip_prefix("DISABLED").unwrap_or(&filename));
            match current_path.parent() {
                Some(parent) => {
                    let new_path = parent.join(&new_filename);
                    if apply_renames {
                        match fs::rename(&current_path, &new_path) {
                            Ok(_) => {
                                current_path = new_path;
                                renamed += 1;
                            }
                            Err(e) => {
                                eprintln!("[scan] failed to rename '{}': {}", filename, e);
                                errors += 1;
                                walker.skip_current_dir();
                                continue;
                            }
                        }
                    } else {
                        renamed += 1;
                    }
                }
                None => {
                    errors += 1;
                    walker.skip_current_dir();
                    continue;
                }
            }
        }

        if !has_ini_file(&current_path) {
            continue; // Not a mod folder itself — let WalkDir descend into its children.
        }

        walker.skip_current_dir(); // Mod folders are leaves — don't look inside for nested mods.
        processed += 1;
        on_progress(processed, &current_path.display().to_string());

        // The .ini told us where a mod is; this decides which folder *is* the mod. See
        // resolve_mod_root — a wrapper folder carries the name a person recognises.
        let current_path = resolve_mod_root(current_path, base_mods_path, maps);

        let relative_path = match current_path.strip_prefix(base_mods_path) {
            Ok(p) => p,
            Err(_) => {
                errors += 1;
                continue;
            }
        };

        let relative_filename = relative_path.file_name().unwrap_or_default().to_string_lossy();
        let clean_filename = relative_filename.strip_prefix(DISABLED_PREFIX).unwrap_or(&relative_filename);
        let clean_relative_path = match relative_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join(clean_filename),
            _ => PathBuf::from(clean_filename),
        };

        mods.push(FoundMod {
            folder_name: clean_relative_path.to_string_lossy().replace('\\', "/"),
            path: current_path,
        });
    }

    WalkOutcome { mods, renamed, errors }
}

/// A folder the next scan would move, and where to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMove {
    pub from: String,
    pub to: String,
}

/// Works out everything a scan would do, and does none of it.
///
/// Reads the same folders, applies the same deduction and the same destination rules as `run_scan`,
/// then reports rather than writes: no rename, no database row, no folder moved. Worth having because
/// a scan rewrites the layout of a whole mods library in one pass and nothing undoes it.
pub fn plan_scan(conn: &Connection, base_mods_path: &Path) -> Result<Vec<PlannedMove>, String> {
    if !base_mods_path.is_dir() {
        return Err(format!(
            "Mods directory path is not a valid directory: {}",
            base_mods_path.display()
        ));
    }

    let maps = fetch_deduction_maps(conn).map_err(|e| e.to_string())?;
    let found = walk_mod_folders(base_mods_path, &maps, false, |_, _| {});

    let mut planned = Vec::new();
    for mod_folder in found.mods {
        let existing: Option<(Option<i64>, Option<i64>, Option<i64>)> = conn
            .query_row(
                "SELECT agent_id, category_id, category_item_id FROM mods WHERE folder_name = ?1",
                params![mod_folder.folder_name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        // Same rule the scan follows: a known mod keeps its assignment unless it has none yet, in
        // which case deduction gets another go at it.
        let (agent_id, category_id, category_item_id) = match existing {
            Some((Some(agent_id), category_id, item_id)) => (Some(agent_id), category_id, item_id),
            Some((None, category_id, item_id)) => {
                let deduced = deduce_mod_info(&mod_folder.path, base_mods_path, &maps);
                if deduced.agent_id.is_some() {
                    (deduced.agent_id, None, None)
                } else {
                    (None, category_id, item_id)
                }
            }
            None => {
                let deduced = deduce_mod_info(&mod_folder.path, base_mods_path, &maps);
                (deduced.agent_id, deduced.category_id, deduced.category_item_id)
            }
        };

        let destination = crate::mods::planned_folder_name(
            conn,
            &mod_folder.folder_name,
            agent_id,
            category_id,
            category_item_id,
        )?;

        if destination != mod_folder.folder_name {
            planned.push(PlannedMove { from: mod_folder.folder_name, to: destination });
        }
    }

    Ok(planned)
}

pub fn run_scan(
    conn: &mut Connection,
    base_mods_path: &Path,
    on_progress: impl FnMut(usize, &str),
) -> Result<String, String> {
    if !base_mods_path.is_dir() {
        return Err(format!("Mods directory path is not a valid directory: {}", base_mods_path.display()));
    }

    let maps = fetch_deduction_maps(conn).map_err(|e| e.to_string())?;

    let found = walk_mod_folders(base_mods_path, &maps, true, on_progress);
    let processed = found.mods.len();
    let renamed = found.renamed;
    let mut errors = found.errors;

    let mut found_folder_names = HashSet::<String>::new();
    let mut added = 0usize;
    let mut remapped = 0usize;
    let mut moved = 0usize;

    // Every mod seen, with where it belongs: (id, agent, category, item). Applied after the walk
    // rather than during it, because moving a folder mid-walk can drop it into a directory WalkDir
    // has not reached yet, which would then discover the same mod a second time.
    let mut placements: Vec<(i64, Option<i64>, Option<i64>, Option<i64>)> = Vec::new();

    for mod_folder in &found.mods {
        let current_path = &mod_folder.path;
        let clean_relative_path_str = mod_folder.folder_name.clone();

        found_folder_names.insert(clean_relative_path_str.clone());

        let existing: Option<(i64, Option<i64>, Option<i64>, Option<i64>)> = conn
            .query_row(
                "SELECT id, agent_id, category_id, category_item_id FROM mods WHERE folder_name = ?1",
                params![clean_relative_path_str],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .ok();

        match existing {
            None => {
                let deduced = deduce_mod_info(current_path, base_mods_path, &maps);
                let insert_result = conn.execute(
                    "INSERT INTO mods (agent_id, category_id, category_item_id, name, folder_name, image_filename, author)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        deduced.agent_id,
                        deduced.category_id,
                        deduced.category_item_id,
                        deduced.name,
                        clean_relative_path_str,
                        deduced.image_filename,
                        deduced.author,
                    ],
                );
                match insert_result {
                    Ok(_) => {
                        added += 1;
                        placements.push((
                            conn.last_insert_rowid(),
                            deduced.agent_id,
                            deduced.category_id,
                            deduced.category_item_id,
                        ));
                    }
                    Err(e) => {
                        eprintln!("[scan] failed to insert mod '{}': {}", clean_relative_path_str, e);
                        errors += 1;
                    }
                }
            }
            // Already known but still unmapped to an agent — re-run deduction in case it can be
            // matched now (e.g. the agent was added, or an alias was added, after this mod was
            // first scanned).
            Some((mod_id, None, category_id, category_item_id)) => {
                let deduced = deduce_mod_info(current_path, base_mods_path, &maps);
                if deduced.agent_id.is_some() {
                    remapped += 1;
                    placements.push((mod_id, deduced.agent_id, None, None));
                } else {
                    placements.push((mod_id, None, category_id, category_item_id));
                }
            }
            // Already mapped. Still queued for placement: its assignment has not changed, but the
            // folder layout may have, and that is exactly what needs correcting on an upgrade.
            Some((mod_id, agent_id, category_id, category_item_id)) => {
                placements.push((mod_id, agent_id, category_id, category_item_id));
            }
        }
    }

    // Bring every folder into line with where its assignment says it belongs. update_mod_category is
    // a no-op when the mod is already in the right place, so this only touches what is misplaced —
    // which on an upgrade is everything, since agent mods used to sit at <slug>/ rather than
    // agents/<slug>/. It also owns the DISABLED_ prefix and the destination-exists checks, so
    // routing through it keeps the scan from growing its own second copy of that logic.
    for (mod_id, agent_id, category_id, category_item_id) in placements {
        match update_mod_category(conn, base_mods_path, mod_id, agent_id, category_id, category_item_id) {
            Ok(updated) => {
                // The prune below only knows the pre-move name, so a moved mod has to declare its
                // new one or it is deleted as "missing" by the very scan that moved it.
                if !found_folder_names.contains(&updated.folder_name) {
                    moved += 1;
                }
                found_folder_names.insert(updated.folder_name);
            }
            Err(e) => {
                eprintln!("[scan] failed to place mod {}: {}", mod_id, e);
                errors += 1;
            }
        }
    }

    let existing_folder_names: Vec<String> = {
        let mut stmt = conn.prepare("SELECT folder_name FROM mods").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    let mut pruned = 0usize;
    for folder_name in existing_folder_names {
        if !found_folder_names.contains(&folder_name) {
            conn.execute("DELETE FROM mods WHERE folder_name = ?1", params![folder_name])
                .map_err(|e| e.to_string())?;
            pruned += 1;
        }
    }

    Ok(format!(
        "Processed {} mod folders.\nAdded {} new mods.\nMapped {} mods to an agent.\nMoved {} folders into place.\nPruned {} missing mods.\nRenamed {} folders.\n{} errors.",
        processed, added, remapped, moved, pruned, renamed, errors
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();

        conn.execute("INSERT INTO agents (name, slug, is_builtin) VALUES ('Ellen', 'ellen', 1)", [])
            .unwrap();
        let ellen_id = conn.last_insert_rowid();
        for alias in ["ellen", "ellen joe", "ellenjoe", "joe"] {
            conn.execute(
                "INSERT INTO agent_aliases (agent_id, alias) VALUES (?1, ?2)",
                params![ellen_id, alias],
            )
            .unwrap();
        }

        conn.execute("INSERT INTO categories (name, slug) VALUES ('Enemies', 'enemies')", [])
            .unwrap();

        conn
    }

    /// Builds a fresh synthetic mods folder under the OS temp dir covering: agent match via
    /// parent folder name, category fallback match, DISABLED-prefix rename fixup (both the
    /// broken "DISABLED" and already-correct "DISABLED_" cases), an excluded-ini-only folder
    /// that must NOT be treated as a mod, and a folder that matches nothing at all.
    fn build_test_mods_dir() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("eous_modify_scanner_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);

        let ellen_skin = base.join("Ellen").join("EllenSkin_v2");
        fs::create_dir_all(&ellen_skin).unwrap();
        fs::write(ellen_skin.join("mod.ini"), "[Mod]\nName = My Ellen Skin\nAuthor = TestAuthor\n").unwrap();

        let enemy_reskin = base.join("Enemies").join("RandomReskin");
        fs::create_dir_all(&enemy_reskin).unwrap();
        fs::write(enemy_reskin.join("mod.ini"), "").unwrap();

        let broken_prefix = base.join("DISABLEDBrokenPrefix");
        fs::create_dir_all(&broken_prefix).unwrap();
        fs::write(broken_prefix.join("mod.ini"), "").unwrap();

        let already_correct = base.join("DISABLED_AlreadyCorrect");
        fs::create_dir_all(&already_correct).unwrap();
        fs::write(already_correct.join("mod.ini"), "").unwrap();

        let no_hints = base.join("RandomModWithNoHints");
        fs::create_dir_all(&no_hints).unwrap();
        fs::write(no_hints.join("mod.ini"), "").unwrap();

        let excluded_only = base.join("ExcludedOnly");
        fs::create_dir_all(&excluded_only).unwrap();
        fs::write(excluded_only.join("region.ini"), "").unwrap();

        base
    }

    /// A scratch mods directory of its own, so these can run in parallel.
    fn temp_base(label: &str) -> PathBuf {
        static BASE_COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = BASE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir()
            .join(format!("eous_scan_{}_{}_{}", label, std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }
    /// A wrapper folder holding one generically named mod folder: the wrapper is the mod, because it
    /// carries the only name that identifies it.
    /// The whole value of a preview is that it tells the truth, so this asserts the prediction against
    /// what the scan then actually does — every move it promised, and no move it did not.
    #[test]
    fn the_preview_matches_what_the_scan_does() {
        let mut conn = setup_test_db();
        let base = temp_base("preview");

        // A spread of the arrangements the placement rules have to handle: a named wrapper, a legacy
        // top-level agent folder, a retired category folder, and something with no clue at all.
        for path in [
            "Ellen Shining Eridu/skin01",
            "ellen/PlainSkin",
            "enemies/RandomReskin",
            "NoHintsAtAll",
        ] {
            let dir = base.join(path);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("mod.ini"), "").unwrap();
        }

        let predicted = plan_scan(&conn, &base).expect("preview should succeed");
        assert!(!predicted.is_empty(), "this fixture is meant to need moving");

        let before: Vec<String> = {
            let mut names: Vec<String> = predicted.iter().map(|m| m.from.clone()).collect();
            names.sort();
            names
        };
        let mut promised: Vec<(String, String)> =
            predicted.into_iter().map(|m| (m.from, m.to)).collect();
        promised.sort();

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        // Every promised destination should now hold the mod, and every promised source should be gone.
        for (from, to) in &promised {
            assert!(
                base.join(to).is_dir() || base.join(to).parent().map(|p| p.is_dir()).unwrap_or(false),
                "promised destination {to} does not exist"
            );
            let landed: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM mods WHERE folder_name = ?1",
                    params![to],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(landed, 1, "the scan did not put a mod at the promised {to} (from {from})");
        }

        for from in &before {
            let still_there: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM mods WHERE folder_name = ?1",
                    params![from],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(still_there, 0, "{from} was promised a move but is still recorded there");
        }

        // And nothing moved that the preview did not mention. Note the fixture deliberately includes
        // a mod that is already in the right place (enemies/RandomReskin, which the Enemies category
        // claims), so "every mod is at a promised destination" would be the wrong assertion — a
        // preview promising a move for a mod that needs none would be just as wrong as missing one.
        let final_names: Vec<String> = {
            let mut stmt = conn.prepare("SELECT folder_name FROM mods ORDER BY folder_name").unwrap();
            let rows = stmt.query_map([], |row| row.get(0)).unwrap();
            let mut v: Vec<String> = rows.collect::<Result<_, _>>().unwrap();
            v.sort();
            v
        };
        let promised_destinations: Vec<&String> = promised.iter().map(|(_, to)| to).collect();
        for name in &final_names {
            let was_predicted = promised_destinations.contains(&name);
            let stayed_put = !before.contains(name);
            assert!(
                was_predicted || stayed_put,
                "{name} is neither a predicted destination nor a mod that stayed where it was"
            );
        }
        assert_eq!(final_names.len(), 4, "every fixture mod should still be recorded exactly once");

        // Running the preview again on the settled library should promise nothing.
        let after = plan_scan(&conn, &base).expect("second preview should succeed");
        assert!(after.is_empty(), "a settled library should have nothing left to move: {after:?}");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_named_wrapper_becomes_the_mod() {
        let mut conn = setup_test_db();
        let base = temp_base("wrapper");
        let inner = base.join("Ellen Shining Eridu").join("skin01");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let (folder_name, name): (String, String) = conn
            .query_row("SELECT folder_name, name FROM mods", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("one mod should be recorded");
        assert_eq!(folder_name, "agents/ellen/Ellen Shining Eridu");
        assert_eq!(name, "Ellen Shining Eridu", "the name should be the wrapper's, not skin01");

        fs::remove_dir_all(&base).ok();
    }

    /// The property that makes climbing safe. A folder holding several mods is a grouping folder, and
    /// treating it as one mod would give every mod inside it a single identity.
    #[test]
    fn a_folder_of_several_mods_is_never_treated_as_one() {
        let mut conn = setup_test_db();
        let base = temp_base("grouping");
        for child in ["EllenA", "EllenB", "EllenC"] {
            let dir = base.join("My Ellen Collection").join(child);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("mod.ini"), "").unwrap();
        }

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM mods", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 3, "each mod in a grouping folder stays its own mod");

        fs::remove_dir_all(&base).ok();
    }

    /// A wrapper with its own .ini is a mod in its own right, with a variant nested inside. Climbing
    /// past it would swallow the outer mod's own files.
    #[test]
    fn a_wrapper_with_its_own_ini_is_not_climbed_past() {
        let mut conn = setup_test_db();
        let base = temp_base("own_ini");
        let outer = base.join("Ellen Outfit");
        let inner = outer.join("variant");
        fs::create_dir_all(&inner).unwrap();
        fs::write(outer.join("mod.ini"), "").unwrap();
        fs::write(inner.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let folders: Vec<String> = {
            let mut stmt = conn.prepare("SELECT folder_name FROM mods ORDER BY folder_name").unwrap();
            let rows = stmt.query_map([], |row| row.get(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(folders, vec!["agents/ellen/Ellen Outfit".to_string()]);

        fs::remove_dir_all(&base).ok();
    }

    /// The pre-agents/ layout put each agent's folder at the top level, holding all of that agent's
    /// mods. Those folders are still on disk until a scan moves them, and one holding a single mod
    /// still must not be mistaken for that mod.
    #[test]
    fn a_legacy_agent_folder_is_not_mistaken_for_a_mod() {
        let mut conn = setup_test_db();
        let base = temp_base("legacy_agent");
        let dir = base.join("ellen").join("SomeSkin");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let folder_name: String = conn
            .query_row("SELECT folder_name FROM mods", [], |row| row.get(0))
            .expect("one mod should be recorded");
        assert_eq!(folder_name, "agents/ellen/SomeSkin", "the mod is SomeSkin, not the ellen folder");

        fs::remove_dir_all(&base).ok();
    }

    /// Same for a retired category folder. `enemies` is no longer defined, so nothing would recognise
    /// it as a category — the explicit list is what keeps it from becoming a mod named "enemies".
    #[test]
    fn a_retired_category_folder_is_not_mistaken_for_a_mod() {
        let mut conn = setup_test_db();
        let base = temp_base("legacy_category");
        let dir = base.join("enemies").join("RandomReskin");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let folder_name: String = conn
            .query_row("SELECT folder_name FROM mods", [], |row| row.get(0))
            .expect("one mod should be recorded");
        assert!(
            folder_name.ends_with("RandomReskin"),
            "the mod is RandomReskin, not the enemies folder (got {folder_name})"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// Running a scan twice must not keep climbing or keep moving.
    #[test]
    fn a_second_scan_changes_nothing() {
        let mut conn = setup_test_db();
        let base = temp_base("idempotent");
        let inner = base.join("Ellen Shining Eridu").join("skin01");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("first scan should succeed");
        let first: String =
            conn.query_row("SELECT folder_name FROM mods", [], |row| row.get(0)).unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("second scan should succeed");
        let second: String =
            conn.query_row("SELECT folder_name FROM mods", [], |row| row.get(0)).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM mods", [], |row| row.get(0)).unwrap();

        assert_eq!(first, second, "a settled mod should not move again");
        assert_eq!(count, 1, "and should not be recorded twice");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scans_synthetic_mods_folder_correctly() {
        let mut conn = setup_test_db();
        let base = build_test_mods_dir();

        let summary = run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");
        println!("{summary}");

        let (agent_id, name): (Option<i64>, String) = conn
            .query_row(
                "SELECT agent_id, name FROM mods WHERE folder_name = 'agents/ellen/EllenSkin_v2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("Ellen mod should exist, matched via parent folder name and moved under agents/");
        assert!(agent_id.is_some(), "Ellen mod should have matched an agent");
        assert_eq!(name, "My Ellen Skin", "mod name should come from the INI's Name field");
        assert!(
            base.join("agents").join("ellen").join("EllenSkin_v2").is_dir(),
            "an agent's mod belongs under agents/<slug>/, not at the folder it was found in"
        );

        let (agent_id2, category_id): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT agent_id, category_id FROM mods WHERE folder_name = 'enemies/RandomReskin'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("Enemies mod should exist, matched via category fallback");
        assert!(agent_id2.is_none());
        assert!(category_id.is_some());
        assert!(
            base.join("enemies").join("RandomReskin").is_dir(),
            "a category's mod sits in the category folder itself, with no synthetic item level"
        );

        let broken_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM mods WHERE folder_name = 'misc/BrokenPrefix'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(broken_count, 1, "DISABLED-prefixed folder should be renamed and stored under its clean name");
        assert!(!base.join("DISABLEDBrokenPrefix").is_dir(), "old incorrectly-prefixed folder should no longer exist");
        assert!(
            base.join("misc").join("DISABLED_BrokenPrefix").is_dir(),
            "the underscore rename and the move to misc/ both apply, and the prefix survives the move"
        );

        let already_correct_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM mods WHERE folder_name = 'misc/AlreadyCorrect'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(already_correct_count, 1, "already-correctly-prefixed DISABLED_ folder should scan without a rename");

        let excluded_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM mods WHERE folder_name LIKE '%ExcludedOnly%'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(excluded_count, 0, "a folder containing only an excluded .ini (region.ini) must not be treated as a mod");

        let (agent_id3, category_id3): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT agent_id, category_id FROM mods WHERE folder_name = 'misc/RandomModWithNoHints'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("uncategorized mod should still be recorded");
        assert!(agent_id3.is_none());
        assert!(category_id3.is_none());
        assert!(
            base.join("misc").join("RandomModWithNoHints").is_dir(),
            "a mod matching nothing belongs in misc/, not loose at the top of the mods folder"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rescan_maps_previously_unmatched_mod_once_its_agent_exists() {
        let mut conn = setup_test_db();

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("eous_modify_scanner_remap_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);
        // The named wrapper holds a generically named inner folder — the arrangement a downloaded
        // mod almost always arrives in. The .ini is in the inner one; the outer one is the mod.
        let mod_dir = base.join("SomeAstraFolder").join("AstraSkin");
        fs::create_dir_all(&mod_dir).unwrap();
        fs::write(mod_dir.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("first scan should succeed");
        // Unmatched, so it goes to misc/ — but as the wrapper, so the name that identifies it is
        // still there for the rescan below to read.
        let (mod_id, agent_id): (i64, Option<i64>) = conn
            .query_row(
                "SELECT id, agent_id FROM mods WHERE folder_name = 'misc/SomeAstraFolder'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("mod should be recorded, unmatched, as misc/SomeAstraFolder");
        assert!(agent_id.is_none(), "no 'Astra' agent exists yet, so it should be unmapped");
        assert!(
            base.join("misc").join("SomeAstraFolder").join("AstraSkin").join("mod.ini").is_file(),
            "the wrapper moves as a whole, with what is inside it left arranged as it was"
        );

        conn.execute("INSERT INTO agents (name, slug, is_builtin) VALUES ('Astra', 'astra', 1)", [])
            .unwrap();
        let astra_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO agent_aliases (agent_id, alias) VALUES (?1, 'astra')",
            params![astra_id],
        )
        .unwrap();

        let summary = run_scan(&mut conn, &base, |_, _| {}).expect("second scan should succeed");
        println!("{summary}");

        let (new_id, new_agent_id, new_folder_name): (i64, Option<i64>, String) = conn
            .query_row(
                "SELECT id, agent_id, folder_name FROM mods WHERE folder_name = 'agents/astra/SomeAstraFolder'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("mod should now be found at its agent-scoped folder path");
        assert_eq!(new_id, mod_id, "remapping should update the existing row, not create a new one");
        assert_eq!(new_agent_id, Some(astra_id), "mod should now be mapped to the Astra agent");
        assert!(
            base.join("agents").join("astra").join("SomeAstraFolder").join("AstraSkin").is_dir(),
            "the whole wrapper moves under the agent, keeping the name that identifies it"
        );
        assert_ne!(new_folder_name, "misc/SomeAstraFolder");

        fs::remove_dir_all(&base).ok();
    }
}
