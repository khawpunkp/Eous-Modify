pub mod archive;
pub mod deduce;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
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
fn walk_mod_folders(
    base_mods_path: &Path,
    maps: &DeductionMaps,
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

        // Fix up a `DISABLED` (missing underscore) prefix before classifying the folder, so the rest
        // of the walk works from one spelling.
        if filename.starts_with("DISABLED") && !filename.starts_with(DISABLED_PREFIX) {
            let new_filename = format!("{}{}", DISABLED_PREFIX, filename.strip_prefix("DISABLED").unwrap_or(&filename));
            match current_path.parent() {
                Some(parent) => {
                    let new_path = parent.join(&new_filename);
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

/// The folder a mod arrived inside, when that folder is a pack rather than part of the layout.
///
/// Read from the mod's own relative path rather than from disk, and read before the placement pass
/// runs: placement moves each mod up to `agents/<slug>/` or `<category>/`, leaving the folder they
/// shared behind and empty, so by the end of a scan there is nothing left to notice.
fn pack_folder_of(folder_name: &str, maps: &DeductionMaps) -> Option<String> {
    let parent = Path::new(folder_name).parent()?;
    if parent.as_os_str().is_empty() || is_structural_folder(parent, maps) {
        return None;
    }
    Some(parent.to_string_lossy().replace('\\', "/"))
}

/// Puts each pack of mods into a group named after the folder they arrived in, and reports how many
/// mods that took in along with how many attempts failed.
///
/// Some mods are a folder of parts rather than one mod — an "Icon Sticker" holding a "Character Menu"
/// and a "Rect&Circle", each with its own .ini. `resolve_mod_root` refuses to merge those into one mod
/// deliberately: nothing on disk tells a pack of parts apart from a folder of unrelated alternatives,
/// and merging the second kind would leave a folder of separate skins sharing a single switch. A group
/// gives what was wanted without that cost — the parts stay separate mods, gain one toggle between
/// them, and keep the name of the folder they came in.
///
/// A mod already in a group is left alone. That group is an arrangement someone made, by hand or on an
/// earlier scan, and not this pass's to rewrite. Where the name is already taken the members join that
/// group rather than a second one of the same name appearing, which is what stops a rescan from
/// multiplying groups for a pack whose folder is still on disk.
fn group_packs(
    conn: &mut Connection,
    base_mods_path: &Path,
    packs: HashMap<String, Vec<i64>>,
) -> (usize, usize) {
    let mut grouped = 0usize;
    let mut errors = 0usize;

    for (pack_path, mod_ids) in packs {
        // The last folder in the path is the pack's own name; the rest is only where it happened to sit.
        let name = pack_path.rsplit('/').next().unwrap_or(&pack_path).to_string();

        let free: Vec<i64> = mod_ids
            .into_iter()
            .filter(|mod_id| {
                conn.query_row(
                    "SELECT COUNT(*) FROM mod_group_members WHERE mod_id = ?1",
                    params![mod_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count == 0)
                .unwrap_or(false)
            })
            .collect();
        if free.is_empty() {
            continue;
        }

        let existing: Option<i64> = conn
            .query_row("SELECT id FROM mod_groups WHERE name = ?1", params![name], |row| row.get(0))
            .ok();

        match existing {
            Some(group_id) => {
                for mod_id in free {
                    match crate::mod_groups::add_member(conn, base_mods_path, group_id, mod_id) {
                        Ok(_) => grouped += 1,
                        Err(e) => {
                            eprintln!("[scan] failed to add mod {} to group '{}': {}", mod_id, name, e);
                            errors += 1;
                        }
                    }
                }
            }
            // create_group's own floor, and the right one: a group of one says nothing the mod does
            // not already say by itself.
            None if free.len() < 2 => continue,
            None => {
                let size = free.len();
                match crate::mod_groups::create_group(conn, base_mods_path, &name, None, &free) {
                    Ok(_) => grouped += size,
                    Err(e) => {
                        eprintln!("[scan] failed to group the mods in '{}': {}", pack_path, e);
                        errors += 1;
                    }
                }
            }
        }
    }

    (grouped, errors)
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

    let found = walk_mod_folders(base_mods_path, &maps, on_progress);
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

    // How many mods each non-layout folder holds. Counted over the whole walk first, because one mod
    // in such a folder is a wrapper that resolve_mod_root has already folded into the mod itself,
    // while two or more make it a pack — and which of the two it is cannot be known until every mod
    // has been seen.
    let mut pack_sizes: HashMap<String, usize> = HashMap::new();
    for mod_folder in &found.mods {
        if let Some(pack) = pack_folder_of(&mod_folder.folder_name, &maps) {
            *pack_sizes.entry(pack).or_default() += 1;
        }
    }
    let mut pack_members: HashMap<String, Vec<i64>> = HashMap::new();

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

        let mod_id = match existing {
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
                        let mod_id = conn.last_insert_rowid();
                        placements.push((
                            mod_id,
                            deduced.agent_id,
                            deduced.category_id,
                            deduced.category_item_id,
                        ));
                        Some(mod_id)
                    }
                    Err(e) => {
                        eprintln!("[scan] failed to insert mod '{}': {}", clean_relative_path_str, e);
                        errors += 1;
                        None
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
                Some(mod_id)
            }
            // Already mapped. Still queued for placement: its assignment has not changed, but the
            // folder layout may have, and that is exactly what needs correcting on an upgrade.
            Some((mod_id, agent_id, category_id, category_item_id)) => {
                placements.push((mod_id, agent_id, category_id, category_item_id));
                Some(mod_id)
            }
        };

        // Noted here rather than after the placement pass, which moves every mod out of the folder
        // they shared. One mod in a folder is a wrapper and already part of the mod; two or more are
        // a pack, and belong in a group together.
        if let (Some(mod_id), Some(pack)) = (mod_id, pack_folder_of(&clean_relative_path_str, &maps)) {
            if pack_sizes.get(&pack).copied().unwrap_or(0) > 1 {
                pack_members.entry(pack).or_default().push(mod_id);
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

    // After placement, since a group says nothing about where a mod sits on disk, and before the
    // prune, so a pack's members are still there to be grouped.
    let (grouped, group_errors) = group_packs(conn, base_mods_path, pack_members);
    errors += group_errors;

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
        "Processed {} mod folders.\nAdded {} new mods.\nMapped {} mods to an agent.\nMoved {} folders into place.\nGrouped {} mods that arrived together.\nPruned {} missing mods.\nRenamed {} folders.\n{} errors.",
        processed, added, remapped, moved, grouped, pruned, renamed, errors
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

    /// Counts the groups a scan left behind, and the members of the first one.
    fn group_summary(conn: &Connection) -> (i64, Option<(String, i64)>) {
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mod_groups", [], |row| row.get(0)).unwrap();
        let first = conn
            .query_row(
                "SELECT g.name, COUNT(m.mod_id) FROM mod_groups g
                 LEFT JOIN mod_group_members m ON m.group_id = g.id
                 GROUP BY g.id ORDER BY g.id LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        (count, first)
    }

    #[test]
    fn a_folder_of_several_mods_becomes_a_group() {
        // "Icon Sticker" holding two mods, each with its own .ini. They stay two mods, because nothing
        // here says they are one, but they arrived together and get a group so they can be switched
        // together.
        let mut conn = setup_test_db();
        let base = temp_base("pack");
        for path in ["Icon Sticker/Character Menu", "Icon Sticker/Rect and Circle"] {
            let dir = base.join(path);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("mod.ini"), "").unwrap();
        }

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let mods: i64 = conn.query_row("SELECT COUNT(*) FROM mods", [], |row| row.get(0)).unwrap();
        assert_eq!(mods, 2, "a pack is still several mods, each toggleable on its own");
        assert_eq!(
            group_summary(&conn),
            (1, Some(("Icon Sticker".to_string(), 2))),
            "both parts belong to one group named after the folder they came in"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_wrapper_around_a_single_mod_makes_no_group() {
        // The wrapper is the mod here, so there is nothing to group it with.
        let mut conn = setup_test_db();
        let base = temp_base("nopack");
        let dir = base.join("Astra Shining Eridu").join("skin01");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mod.ini"), "").unwrap();

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        assert_eq!(group_summary(&conn).0, 0, "one mod is not a pack");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn an_agent_folder_full_of_mods_is_never_grouped() {
        // The folder that matters most to get right: agents/<slug> holds every mod for that agent, and
        // they are unrelated to each other. Reading it as a pack would put one switch on all of them.
        let mut conn = setup_test_db();
        let base = temp_base("agentfolder");
        for path in ["agents/ellen/Bassist Ellen", "agents/ellen/Ellen Casual"] {
            let dir = base.join(path);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("mod.ini"), "").unwrap();
        }

        run_scan(&mut conn, &base, |_, _| {}).expect("scan should succeed");

        let mods: i64 = conn.query_row("SELECT COUNT(*) FROM mods", [], |row| row.get(0)).unwrap();
        assert_eq!(mods, 2);
        assert_eq!(group_summary(&conn).0, 0, "a layout folder is not a pack");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_second_scan_does_not_group_a_pack_twice() {
        // These match nothing, so they are filed under misc keeping the folder they arrived in — which
        // means the second scan sees the same pack again and has to recognise it as already handled.
        let mut conn = setup_test_db();
        let base = temp_base("packagain");
        for path in ["Icon Sticker/Character Menu", "Icon Sticker/Rect and Circle"] {
            let dir = base.join(path);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("mod.ini"), "").unwrap();
        }

        run_scan(&mut conn, &base, |_, _| {}).expect("first scan should succeed");
        assert!(base.join("misc").join("Icon Sticker").is_dir(), "this fixture needs the folder to survive");

        run_scan(&mut conn, &base, |_, _| {}).expect("second scan should succeed");

        assert_eq!(
            group_summary(&conn),
            (1, Some(("Icon Sticker".to_string(), 2))),
            "the same pack should join the group it already has, not a second one"
        );

        fs::remove_dir_all(&base).ok();
    }
}
