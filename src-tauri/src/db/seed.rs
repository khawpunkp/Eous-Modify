use std::collections::{HashMap, HashSet};
use std::fs;

use rusqlite::{params, Connection, Transaction};
use serde::Deserialize;
use tauri::{path::BaseDirectory, AppHandle, Manager};

const SETTINGS_KEY_APP_VERSION: &str = "app_version";

#[derive(Deserialize, Debug, Clone)]
struct AgentDefinition {
    name: String,
    /// The character's name in full. Optional: omit it and readers fall back to `name`.
    #[serde(default)]
    full_name: Option<String>,
    slug: String,
    details: Option<String>,
    base_image: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct CharacterCategoryDefinition {
    entities: Vec<AgentDefinition>,
}

#[derive(Deserialize, Debug, Clone)]
struct CategoryItemDefinition {
    name: String,
    slug: String,
    description: Option<String>,
    details: Option<String>,
    base_image: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CategoryDefinition {
    name: String,
    /// Extra strings a folder name might use for this category, beyond its own name and slug.
    #[serde(default)]
    aliases: Vec<String>,
    entities: Vec<CategoryItemDefinition>,
}

type Definitions = HashMap<String, CategoryDefinition>;

/// Lowercase, ASCII-alphanumeric-only slug with single dashes between words.
pub fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in input.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Version-gated sync of `definitions/zzz.toml` into the agents/categories tables.
/// Ported from the pre-rebuild app's `sync_definitions` (its `main.rs` is in this repo's history),
/// split into an agents path and a categories path since the new schema separates the two.
pub fn sync_definitions(conn: &mut Connection, app_handle: &AppHandle) -> Result<(), String> {
    let current_version = app_handle.package_info().version.to_string();


    let resource_path = app_handle
        .path()
        .resolve("definitions/zzz.toml", BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let toml_str = fs::read_to_string(&resource_path).map_err(|e| e.to_string())?;
    let mut root: toml::Value = toml::from_str(&toml_str).map_err(|e| e.to_string())?;
    let table = root
        .as_table_mut()
        .ok_or_else(|| "definitions root is not a table".to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    if let Some(characters_value) = table.remove("characters") {
        let characters_def: CharacterCategoryDefinition = characters_value
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        sync_agents(&tx, &characters_def.entities)?;
    }

    let rest: Definitions = toml::Value::Table(table.clone())
        .try_into()
        .map_err(|e: toml::de::Error| e.to_string())?;
    sync_categories(&tx, &rest)?;

    tx.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SETTINGS_KEY_APP_VERSION, current_version],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn sync_agents(tx: &Transaction, defs: &[AgentDefinition]) -> Result<(), String> {
    let mut seed_slugs = HashSet::new();

    for def in defs {
        seed_slugs.insert(def.slug.clone());

        tx.execute(
            "INSERT INTO agents (name, full_name, slug, details, base_image, is_builtin)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(slug) DO UPDATE SET
                name = excluded.name,
                full_name = excluded.full_name,
                details = excluded.details,
                base_image = excluded.base_image,
                is_builtin = 1",
            params![def.name, def.full_name, def.slug, def.details, def.base_image],
        )
        .map_err(|e| e.to_string())?;

        let agent_id: i64 = tx
            .query_row(
                "SELECT id FROM agents WHERE slug = ?1",
                params![def.slug],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        // Additive-only: never removes an alias a user may have added or kept.
        for alias in &def.aliases {
            tx.execute(
                "INSERT OR IGNORE INTO agent_aliases (agent_id, alias) VALUES (?1, ?2)",
                params![agent_id, alias],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    let existing_builtin_slugs: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT slug FROM agents WHERE is_builtin = 1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    for slug in existing_builtin_slugs {
        if seed_slugs.contains(&slug) {
            continue;
        }
        let has_mods: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM mods m JOIN agents a ON m.agent_id = a.id WHERE a.slug = ?1)",
                params![slug],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !has_mods {
            tx.execute("DELETE FROM agents WHERE slug = ?1", params![slug])
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn sync_categories(tx: &Transaction, defs: &Definitions) -> Result<(), String> {
    for (category_slug, category_def) in defs.iter() {
        tx.execute(
            "INSERT INTO categories (name, slug) VALUES (?1, ?2)
             ON CONFLICT(slug) DO UPDATE SET name = excluded.name",
            params![category_def.name, category_slug],
        )
        .map_err(|e| e.to_string())?;

        let category_id: i64 = tx
            .query_row(
                "SELECT id FROM categories WHERE slug = ?1",
                params![category_slug],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        // Replaced wholesale rather than merged, so removing an alias from the definitions actually
        // removes it. Same reasoning as the item prune below; the table is tiny.
        tx.execute("DELETE FROM category_aliases WHERE category_id = ?1", params![category_id])
            .map_err(|e| e.to_string())?;
        for alias in &category_def.aliases {
            tx.execute(
                "INSERT INTO category_aliases (category_id, alias) VALUES (?1, ?2)
                 ON CONFLICT(category_id, alias) DO NOTHING",
                params![category_id, alias.to_lowercase()],
            )
            .map_err(|e| e.to_string())?;
        }

        let mut seed_slugs = HashSet::new();

        for item in &category_def.entities {
            seed_slugs.insert(item.slug.clone());

            tx.execute(
                "INSERT INTO category_items (category_id, name, slug, description, details, base_image)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(slug) DO UPDATE SET
                    category_id = excluded.category_id,
                    name = excluded.name,
                    description = excluded.description,
                    details = excluded.details,
                    base_image = excluded.base_image",
                params![
                    category_id,
                    item.name,
                    item.slug,
                    item.description,
                    item.details,
                    item.base_image
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        // Permanent catch-all item for mods that aren't tied to any specific seeded item —
        // added to seed_slugs so the prune loop below never deletes it, even with zero mods in it.
        let other_slug = format!("{}-other", category_slug);
        seed_slugs.insert(other_slug.clone());
        tx.execute(
            "INSERT INTO category_items (category_id, name, slug) VALUES (?1, ?2, ?3)
             ON CONFLICT(slug) DO UPDATE SET category_id = excluded.category_id, name = excluded.name",
            params![category_id, format!("Other {}", category_def.name), other_slug],
        )
        .map_err(|e| e.to_string())?;

        let existing_slugs: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT slug FROM category_items WHERE category_id = ?1")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![category_id], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        };

        for slug in existing_slugs {
            if seed_slugs.contains(&slug) {
                continue;
            }
            let has_mods: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM mods m JOIN category_items ci ON m.category_item_id = ci.id WHERE ci.slug = ?1)",
                    params![slug],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !has_mods {
                tx.execute("DELETE FROM category_items WHERE slug = ?1", params![slug])
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    prune_undefined_categories(tx, defs)
}

/// Drops whole categories the definitions no longer describe.
///
/// The item prune above only ever looks *inside* a category that still exists, so a category dropped
/// from the definitions used to live on forever — which is how NPCs, Enemies, Weapons and Objects
/// outlasted the rebuild that stopped defining them, on every install that had ever seen them.
///
/// Deliberately not guarded by "has mods", unlike the item prune. Retiring a category is a decision
/// about how mods are organised, and refusing to carry it out wherever it would actually change
/// something makes it useless. Nothing is lost: the mods' foreign keys are ON DELETE SET NULL, so
/// they simply become uncategorised and turn up on the Other page, and their folders on disk are not
/// touched.
fn prune_undefined_categories(tx: &Transaction, defs: &Definitions) -> Result<(), String> {
    let existing: Vec<(i64, String)> = {
        let mut stmt = tx.prepare("SELECT id, slug FROM categories").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    for (id, slug) in existing {
        if defs.contains_key(&slug) {
            continue;
        }
        // category_items and category_aliases cascade; mods fall back to NULL.
        tx.execute("DELETE FROM categories WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        eprintln!("[seed] removed category '{slug}', no longer in the definitions");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        conn
    }

    /// A user may add a custom agent (e.g. "Astra") before it exists as a built-in one — slugify()
    /// is deterministic, so a later-added base agent with the same name shares that same slug, and
    /// the ON CONFLICT(slug) upsert merges into the same row rather than creating a duplicate. The
    /// alias insert is additive-only ("INSERT OR IGNORE", never a DELETE), so the user's own alias
    /// survives right alongside whatever alias the base definition itself declares.
    #[test]
    fn merges_custom_agent_into_newly_added_base_agent_keeping_aliases() {
        let mut conn = setup();

        conn.execute("INSERT INTO agents (name, slug, is_builtin) VALUES ('Astra', 'astra', 0)", [])
            .unwrap();
        let custom_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO agent_aliases (agent_id, alias) VALUES (?1, 'my-custom-alias')",
            params![custom_id],
        )
        .unwrap();

        let defs = vec![AgentDefinition {
            name: "Astra".to_string(),
            full_name: Some("Astra Yao".to_string()),
            slug: "astra".to_string(),
            details: Some(r#"{"rank":"S"}"#.to_string()),
            base_image: Some("astra_base.jpg".to_string()),
            aliases: vec!["astra".to_string()],
        }];

        let tx = conn.transaction().unwrap();
        sync_agents(&tx, &defs).expect("sync should succeed");
        tx.commit().unwrap();

        let (id, name, is_builtin, details, base_image): (i64, String, i64, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT id, name, is_builtin, details, base_image FROM agents WHERE slug = 'astra'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();

        assert_eq!(id, custom_id, "should merge into the existing row, not create a second one");
        assert_eq!(name, "Astra");
        assert_eq!(is_builtin, 1, "should become a built-in agent");
        assert_eq!(details.as_deref(), Some(r#"{"rank":"S"}"#));
        assert_eq!(base_image.as_deref(), Some("astra_base.jpg"));

        let aliases: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT alias FROM agent_aliases WHERE agent_id = ?1 ORDER BY alias")
                .unwrap();
            stmt.query_map(params![custom_id], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            aliases,
            vec!["astra".to_string(), "my-custom-alias".to_string()],
            "user's own alias must survive, and the base definition's own alias gets added too"
        );
    }
}

#[cfg(test)]
mod category_tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        conn
    }

    fn definition(name: &str, aliases: &[&str]) -> CategoryDefinition {
        CategoryDefinition {
            name: name.to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            entities: Vec::new(),
        }
    }

    /// The four retired categories outlived the rebuild because the item prune only ever looked
    /// inside categories that still existed. Their mods have to survive the removal.
    #[test]
    fn drops_categories_the_definitions_no_longer_describe() {
        let mut conn = setup();
        conn.execute("INSERT INTO categories (name, slug) VALUES ('NPCs', 'npcs')", []).unwrap();
        let npcs_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO category_items (category_id, name, slug) VALUES (?1, 'Other NPCs', 'npcs-other')",
            params![npcs_id],
        )
        .unwrap();
        let item_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO mods (category_id, category_item_id, name, folder_name)
             VALUES (?1, ?2, 'Some Mod', 'npcs/npcs-other/SomeMod')",
            params![npcs_id, item_id],
        )
        .unwrap();

        let mut defs: Definitions = HashMap::new();
        defs.insert("ui".to_string(), definition("UI", &[]));

        let tx = conn.transaction().unwrap();
        sync_categories(&tx, &defs).unwrap();
        tx.commit().unwrap();

        let categories: Vec<String> = {
            let mut stmt = conn.prepare("SELECT slug FROM categories ORDER BY slug").unwrap();
            let rows = stmt.query_map([], |row| row.get(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(categories, vec!["ui".to_string()], "npcs should be gone, ui seeded");

        // The mod itself survives, uncategorised — it will show on the Other page.
        let (id, category_id, item): (i64, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT id, category_id, category_item_id FROM mods WHERE folder_name = 'npcs/npcs-other/SomeMod'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("the mod must not be deleted along with its category");
        assert!(id > 0);
        assert_eq!(category_id, None);
        assert_eq!(item, None);
    }

    /// Aliases are replaced rather than merged, so dropping one from the definitions takes effect.
    #[test]
    fn replaces_category_aliases_on_resync() {
        let mut conn = setup();

        let mut defs: Definitions = HashMap::new();
        defs.insert("bangboos".to_string(), definition("Bangboos", &["bangboo", "boo"]));

        let tx = conn.transaction().unwrap();
        sync_categories(&tx, &defs).unwrap();
        tx.commit().unwrap();

        let read = |conn: &Connection| -> Vec<String> {
            let mut stmt = conn.prepare("SELECT alias FROM category_aliases ORDER BY alias").unwrap();
            let rows = stmt.query_map([], |row| row.get(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(read(&conn), vec!["bangboo".to_string(), "boo".to_string()]);

        defs.insert("bangboos".to_string(), definition("Bangboos", &["boo"]));
        let tx = conn.transaction().unwrap();
        sync_categories(&tx, &defs).unwrap();
        tx.commit().unwrap();

        assert_eq!(read(&conn), vec!["boo".to_string()], "the dropped alias should be gone");
    }
}
