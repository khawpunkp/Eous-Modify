use std::collections::{HashMap, HashSet};
use std::fs;

use rusqlite::{params, Connection, Transaction};
use serde::Deserialize;
use tauri::{path::BaseDirectory, AppHandle, Manager};

const SETTINGS_KEY_APP_VERSION: &str = "app_version";

#[derive(Deserialize, Debug, Clone)]
struct AgentDefinition {
    name: String,
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
            "INSERT INTO agents (name, slug, details, base_image, is_builtin)
             VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT(slug) DO UPDATE SET
                name = excluded.name,
                details = excluded.details,
                base_image = excluded.base_image,
                is_builtin = 1",
            params![def.name, def.slug, def.details, def.base_image],
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
