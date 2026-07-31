pub mod schema;
pub mod seed;

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

const DB_FILENAME: &str = "eous-modify.db";

/// The mod-grouping feature (and `mod_group_members`) is brand new as of this version, so it's
/// always safe to drop and let `schema::SCHEMA` below recreate it with the `UNIQUE(mod_id)`
/// constraint — there is no prior release where a real mod group could have existed. Gated by a
/// one-time flag so this doesn't repeat (and destroy real group data) on every future startup.
fn migrate_mod_group_members_unique(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);")?;

    let already_migrated = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'migration_mod_group_members_unique'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if !already_migrated {
        conn.execute("DROP TABLE IF EXISTS mod_group_members", [])?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('migration_mod_group_members_unique', 'true')",
            [],
        )?;
    }

    Ok(())
}

/// Presets were removed outright (not replaced by anything) — drop any existing data rather than
/// leaving orphaned tables `schema::SCHEMA` no longer recreates. Gated the same way as the group
/// migration above so this only runs once per install.
fn migrate_drop_presets(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);")?;

    let already_migrated = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'migration_drop_presets'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if !already_migrated {
        conn.execute("DROP TABLE IF EXISTS preset_mods", [])?;
        conn.execute("DROP TABLE IF EXISTS presets", [])?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('migration_drop_presets', 'true')",
            [],
        )?;
    }

    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The description field was removed from agents/mods entirely (not replaced by anything) — drop
/// the columns rather than leaving them as orphaned dead data. `column_exists` makes this a no-op
/// on a fresh install (the table doesn't exist yet, or already lacks the column). Gated the same
/// way as the migrations above so this only runs once per install.
fn migrate_drop_mod_agent_description(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);")?;

    let already_migrated = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'migration_drop_mod_agent_description'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if !already_migrated {
        if column_exists(conn, "agents", "description")? {
            conn.execute("ALTER TABLE agents DROP COLUMN description", [])?;
        }
        if column_exists(conn, "mods", "description")? {
            conn.execute("ALTER TABLE mods DROP COLUMN description", [])?;
        }
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('migration_drop_mod_agent_description', 'true')",
            [],
        )?;
    }

    Ok(())
}

/// Adds `mod_groups.base_image` to installs created before groups had an image. Naturally
/// idempotent via the column check, so this needs no settings flag — unlike the destructive
/// migrations above, re-running it is a no-op rather than data loss.
fn migrate_add_mod_group_base_image(conn: &Connection) -> rusqlite::Result<()> {
    // Migrations run before schema::SCHEMA, so on a fresh install the table isn't there yet and
    // SCHEMA will create it with the column already present.
    let table_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mod_groups')",
        [],
        |row| row.get(0),
    )?;

    if table_exists && !column_exists(conn, "mod_groups", "base_image")? {
        conn.execute("ALTER TABLE mod_groups ADD COLUMN base_image TEXT", [])?;
    }

    Ok(())
}

/// Clears out custom agents with a blank name. Their slug is derived from the name, so a blank one
/// slugifies to "" — leaving an agent with no reachable `/agents/[slug]` route and therefore no way
/// to get at its own Delete button. `create_agent` now rejects these, but any already written by an
/// earlier build would otherwise be stuck in the table forever (and squatting the one empty slug
/// the UNIQUE constraint allows). Any mods filed under it survive: the FK is ON DELETE SET NULL, so
/// they simply become uncategorized and show up on the Other page.
///
/// Naturally idempotent — a no-op once there's nothing blank left — so no settings flag needed.
fn repair_unnamed_agents(conn: &Connection) -> rusqlite::Result<()> {
    let table_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'agents')",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(());
    }

    let removed = conn.execute("DELETE FROM agents WHERE is_builtin = 0 AND TRIM(name) = ''", [])?;
    if removed > 0 {
        eprintln!("[db] removed {} unnamed custom agent(s) with no reachable detail page", removed);
    }

    Ok(())
}

pub fn init_db(app_data_dir: &Path) -> rusqlite::Result<Connection> {
    std::fs::create_dir_all(app_data_dir).expect("failed to create app data dir");
    let db_path = app_data_dir.join(DB_FILENAME);
    let conn = Connection::open(db_path)?;
    migrate_mod_group_members_unique(&conn)?;
    migrate_drop_presets(&conn)?;
    migrate_drop_mod_agent_description(&conn)?;
    migrate_add_mod_group_base_image(&conn)?;
    repair_unnamed_agents(&conn)?;
    conn.execute_batch(schema::SCHEMA)?;
    Ok(conn)
}
