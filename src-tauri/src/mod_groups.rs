use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::models::{ModGroupMember, ModGroupWithMembers};
use crate::mods::{is_mod_enabled, toggle_mod};

fn fetch_members(conn: &Connection, base_mods_path: &Path, group_id: i64) -> Result<Vec<ModGroupMember>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.name, m.folder_name FROM mod_group_members mgm
             JOIN mods m ON m.id = mgm.mod_id
             WHERE mgm.group_id = ?1
             ORDER BY m.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|e| e.to_string())?;
    let raw: Vec<(i64, String, String)> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;

    Ok(raw
        .into_iter()
        .map(|(mod_id, name, folder_name)| {
            let is_enabled = is_mod_enabled(base_mods_path, &folder_name).unwrap_or(false);
            ModGroupMember { mod_id, name, folder_name, is_enabled }
        })
        .collect())
}

fn group_is_enabled(members: &[ModGroupMember]) -> bool {
    !members.is_empty() && members.iter().all(|m| m.is_enabled)
}

pub fn list_groups(conn: &Connection, base_mods_path: &Path) -> Result<Vec<ModGroupWithMembers>, String> {
    let groups: Vec<(i64, String, Option<String>)> = {
        let mut stmt = conn
            .prepare("SELECT id, name, base_image FROM mod_groups ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    groups
        .into_iter()
        .map(|(id, name, base_image)| {
            let members = fetch_members(conn, base_mods_path, id)?;
            let is_enabled = group_is_enabled(&members);
            Ok(ModGroupWithMembers { id, name, base_image, is_enabled, members })
        })
        .collect()
}

pub fn get_group(conn: &Connection, base_mods_path: &Path, group_id: i64) -> Result<ModGroupWithMembers, String> {
    let (name, base_image): (String, Option<String>) = conn
        .query_row(
            "SELECT name, base_image FROM mod_groups WHERE id = ?1",
            params![group_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let members = fetch_members(conn, base_mods_path, group_id)?;
    let is_enabled = group_is_enabled(&members);
    Ok(ModGroupWithMembers { id: group_id, name, base_image, is_enabled, members })
}

pub fn create_group(
    conn: &mut Connection,
    base_mods_path: &Path,
    name: &str,
    base_image: Option<&str>,
    mod_ids: &[i64],
) -> Result<ModGroupWithMembers, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Group name cannot be empty.".to_string());
    }
    let unique_ids: HashSet<i64> = mod_ids.iter().copied().collect();
    if unique_ids.len() < 2 {
        return Err("A group needs at least 2 mods.".to_string());
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for &mod_id in &unique_ids {
        let exists: i64 = tx
            .query_row("SELECT COUNT(*) FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if exists == 0 {
            return Err(format!("Mod ID {} does not exist.", mod_id));
        }
        let already_grouped: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM mod_group_members WHERE mod_id = ?1",
                params![mod_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if already_grouped > 0 {
            return Err(format!("Mod ID {} is already in a group.", mod_id));
        }
    }

    tx.execute(
        "INSERT INTO mod_groups (name, base_image) VALUES (?1, ?2)",
        params![name, base_image],
    )
    .map_err(|e| e.to_string())?;
    let group_id = tx.last_insert_rowid();

    for &mod_id in &unique_ids {
        tx.execute(
            "INSERT INTO mod_group_members (group_id, mod_id) VALUES (?1, ?2)",
            params![group_id, mod_id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    get_group(conn, base_mods_path, group_id)
}

/// Sets both name and image in one write — the group modal edits them together, so splitting this
/// into two commands would just mean two round-trips and a half-applied state on failure.
pub fn update_group(
    conn: &Connection,
    base_mods_path: &Path,
    group_id: i64,
    name: &str,
    base_image: Option<&str>,
) -> Result<ModGroupWithMembers, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Group name cannot be empty.".to_string());
    }
    conn.execute(
        "UPDATE mod_groups SET name = ?1, base_image = ?2 WHERE id = ?3",
        params![name, base_image, group_id],
    )
    .map_err(|e| e.to_string())?;
    get_group(conn, base_mods_path, group_id)
}

pub fn delete_group(conn: &Connection, group_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM mod_groups WHERE id = ?1", params![group_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn add_member(
    conn: &mut Connection,
    base_mods_path: &Path,
    group_id: i64,
    mod_id: i64,
) -> Result<ModGroupWithMembers, String> {
    let already_grouped: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mod_group_members WHERE mod_id = ?1",
            params![mod_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if already_grouped > 0 {
        return Err("This mod is already in a group.".to_string());
    }

    conn.execute(
        "INSERT INTO mod_group_members (group_id, mod_id) VALUES (?1, ?2)",
        params![group_id, mod_id],
    )
    .map_err(|e| e.to_string())?;

    get_group(conn, base_mods_path, group_id)
}

/// Removes one mod from its group, leaving the group in place even if only one member remains.
/// Disbanding is an explicit action (`delete_group`), never a side effect of a removal.
pub fn remove_member(
    conn: &mut Connection,
    base_mods_path: &Path,
    group_id: i64,
    mod_id: i64,
) -> Result<ModGroupWithMembers, String> {
    conn.execute(
        "DELETE FROM mod_group_members WHERE group_id = ?1 AND mod_id = ?2",
        params![group_id, mod_id],
    )
    .map_err(|e| e.to_string())?;

    get_group(conn, base_mods_path, group_id)
}

/// All-on-unless-all-on: if every member is currently enabled, disable all; otherwise enable all.
/// Returns the resulting group-level state.
pub fn toggle_group(conn: &Connection, base_mods_path: &Path, group_id: i64) -> Result<bool, String> {
    let members = fetch_members(conn, base_mods_path, group_id)?;
    if members.is_empty() {
        return Err("Group has no members.".to_string());
    }

    let all_enabled = members.iter().all(|m| m.is_enabled);
    let target_state = !all_enabled;

    for member in &members {
        if member.is_enabled != target_state {
            toggle_mod(base_mods_path, &member.folder_name)?;
        }
    }

    Ok(target_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn setup_test_db_and_dir() -> (Connection, PathBuf) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("eous_modify_groups_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);

        for name in ["ModA", "ModB", "ModC"] {
            fs::create_dir_all(base.join(name)).unwrap();
            fs::write(base.join(name).join("mod.ini"), "").unwrap();
            conn.execute("INSERT INTO mods (name, folder_name) VALUES (?1, ?2)", params![name, name])
                .unwrap();
        }

        (conn, base)
    }

    fn mod_ids(conn: &Connection) -> Vec<i64> {
        let mut stmt = conn.prepare("SELECT id FROM mods ORDER BY name").unwrap();
        stmt.query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    }

    #[test]
    fn create_and_toggle_group_with_mixed_state() {
        let (mut conn, base) = setup_test_db_and_dir();
        let ids = mod_ids(&conn);

        let group = create_group(&mut conn, &base, "My Outfit", None, &ids[0..2]).expect("create should succeed");
        assert!(group.is_enabled, "both members start enabled");

        // Disable ModA on disk -> mixed state.
        toggle_mod(&base, "ModA").unwrap();
        let group = get_group(&conn, &base, group.id).unwrap();
        assert!(!group.is_enabled, "mixed state should not read as enabled");

        // Toggling a mixed-state group should turn everything ON, not off.
        let new_state = toggle_group(&conn, &base, group.id).expect("toggle should succeed");
        assert!(new_state, "toggling a mixed group should enable everything");
        let group = get_group(&conn, &base, group.id).unwrap();
        assert!(group.members.iter().all(|m| m.is_enabled));

        // Toggling an all-enabled group should turn everything OFF.
        let new_state = toggle_group(&conn, &base, group.id).expect("toggle should succeed");
        assert!(!new_state);
        let group = get_group(&conn, &base, group.id).unwrap();
        assert!(group.members.iter().all(|m| !m.is_enabled));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn create_and_update_group_round_trips_base_image() {
        let (mut conn, base) = setup_test_db_and_dir();
        let ids = mod_ids(&conn);

        let group = create_group(&mut conn, &base, "With Image", Some("data:image/png;base64,AAA"), &ids[0..2])
            .expect("create should succeed");
        assert_eq!(group.base_image.as_deref(), Some("data:image/png;base64,AAA"));

        // Re-reading through the list path must surface it too, not just the create return value.
        let listed = list_groups(&conn, &base).unwrap();
        assert_eq!(listed[0].base_image.as_deref(), Some("data:image/png;base64,AAA"));

        let updated = update_group(&conn, &base, group.id, "Renamed", Some("data:image/png;base64,BBB"))
            .expect("update should succeed");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.base_image.as_deref(), Some("data:image/png;base64,BBB"));
        assert_eq!(updated.members.len(), 2, "updating name/image must not disturb membership");

        // Clearing the image back to None must actually persist as NULL.
        let cleared = update_group(&conn, &base, group.id, "Renamed", None).expect("clear should succeed");
        assert_eq!(cleared.base_image, None);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn update_group_rejects_empty_name() {
        let (mut conn, base) = setup_test_db_and_dir();
        let ids = mod_ids(&conn);
        let group = create_group(&mut conn, &base, "Named", None, &ids[0..2]).unwrap();

        assert!(update_group(&conn, &base, group.id, "   ", None).is_err());

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn create_group_rejects_already_grouped_mod() {
        let (mut conn, base) = setup_test_db_and_dir();
        let ids = mod_ids(&conn);

        create_group(&mut conn, &base, "First Group", None, &ids[0..2]).unwrap();
        let result = create_group(&mut conn, &base, "Second Group", None, &[ids[0], ids[2]]);
        assert!(result.is_err(), "should reject a mod that's already in a group");

        fs::remove_dir_all(&base).ok();
    }

    /// Removal is only ever removal: the group survives even at a single member, and disbanding is
    /// left to the explicit Ungroup action. (This replaced an earlier auto-disband-at-one rule.)
    #[test]
    fn remove_member_leaves_the_group_intact_even_at_one_member() {
        let (mut conn, base) = setup_test_db_and_dir();
        let ids = mod_ids(&conn);

        let group = create_group(&mut conn, &base, "Trio", None, &ids).unwrap();
        assert_eq!(group.members.len(), 3);

        let after_first = remove_member(&mut conn, &base, group.id, ids[0]).expect("remove should succeed");
        assert_eq!(after_first.members.len(), 2);

        let after_second = remove_member(&mut conn, &base, group.id, ids[1]).expect("remove should succeed");
        assert_eq!(after_second.members.len(), 1, "a one-member group must persist, not auto-disband");

        let still_there: i64 = conn
            .query_row("SELECT COUNT(*) FROM mod_groups WHERE id = ?1", params![group.id], |row| row.get(0))
            .unwrap();
        assert_eq!(still_there, 1, "the group row itself must survive");

        // And the survivor is still a member, so the group can be topped back up.
        let members: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM mod_group_members WHERE group_id = ?1",
                params![group.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(members, 1);

        fs::remove_dir_all(&base).ok();
    }
}