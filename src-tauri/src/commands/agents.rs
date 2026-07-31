use rusqlite::{params, Connection};
use tauri::State;

use crate::db::seed::slugify;
use crate::models::{AgentInput, AgentWithAliases};
use crate::DbState;

/// An agent's slug is derived from its name and is the only handle the `/agents/[slug]` route has,
/// so a name that slugifies to nothing (blank, or punctuation-only like "???") produces an agent
/// with no reachable detail page — and therefore no way to reach its own Delete button. Reject
/// both cases up front rather than letting an unreachable row into the table.
fn validated_slug(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Agent name cannot be empty.".to_string());
    }
    let slug = slugify(name);
    if slug.is_empty() {
        return Err("Agent name must contain at least one letter or number.".to_string());
    }
    Ok(slug)
}

fn row_to_agent(conn: &Connection, id: i64) -> rusqlite::Result<AgentWithAliases> {
    let (name, slug, details, base_image, is_builtin): (
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
    ) = conn.query_row(
        "SELECT name, slug, details, base_image, is_builtin FROM agents WHERE id = ?1",
        params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )?;

    let mut stmt = conn.prepare("SELECT alias FROM agent_aliases WHERE agent_id = ?1 ORDER BY alias")?;
    let aliases = stmt
        .query_map(params![id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AgentWithAliases {
        id,
        name,
        slug,
        details,
        base_image,
        is_builtin: is_builtin != 0,
        aliases,
    })
}

#[tauri::command]
pub fn list_agents(state: State<DbState>) -> Result<Vec<AgentWithAliases>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let ids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM agents ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    ids.into_iter()
        .map(|id| row_to_agent(&conn, id).map_err(|e| e.to_string()))
        .collect()
}

#[tauri::command]
pub fn get_agent(slug: String, state: State<DbState>) -> Result<AgentWithAliases, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let id: i64 = conn
        .query_row("SELECT id FROM agents WHERE slug = ?1", params![slug], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    row_to_agent(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_agent(input: AgentInput, state: State<DbState>) -> Result<AgentWithAliases, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    let slug = validated_slug(&input.name)?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO agents (name, slug, details, base_image, is_builtin)
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![input.name, slug, input.details, input.base_image],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    for alias in &input.aliases {
        tx.execute(
            "INSERT OR IGNORE INTO agent_aliases (agent_id, alias) VALUES (?1, ?2)",
            params![id, alias],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;

    row_to_agent(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_agent(
    slug: String,
    input: AgentInput,
    state: State<DbState>,
) -> Result<AgentWithAliases, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    let (id, is_builtin): (i64, i64) = conn
        .query_row(
            "SELECT id, is_builtin FROM agents WHERE slug = ?1",
            params![slug],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    // A built-in agent's name/details/base_image come from definitions/zzz.toml and get rewritten
    // by the version-gated seed re-sync, so accepting edits to them would silently lose the change
    // on the next update. Aliases are the one field sync leaves alone (additive-only), so those
    // still apply below.
    if is_builtin == 0 {
        // Validate only on the path that actually writes the name — a built-in's name comes from
        // zzz.toml and is never blank, and its alias-only save shouldn't be blocked by whatever
        // the (disabled) name field happened to submit.
        validated_slug(&input.name)?;
        tx.execute(
            "UPDATE agents SET name = ?1, details = ?2, base_image = ?3 WHERE id = ?4",
            params![input.name, input.details, input.base_image, id],
        )
        .map_err(|e| e.to_string())?;
    }

    // Full-replace diff for user edits (unlike seed-sync's additive-only aliases).
    let existing: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT alias FROM agent_aliases WHERE agent_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![id], |row| row.get(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    for alias in &existing {
        if !input.aliases.contains(alias) {
            tx.execute(
                "DELETE FROM agent_aliases WHERE agent_id = ?1 AND alias = ?2",
                params![id, alias],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    for alias in &input.aliases {
        tx.execute(
            "INSERT OR IGNORE INTO agent_aliases (agent_id, alias) VALUES (?1, ?2)",
            params![id, alias],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;

    row_to_agent(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_agent(slug: String, state: State<DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let (id, is_builtin): (i64, i64) = conn
        .query_row(
            "SELECT id, is_builtin FROM agents WHERE slug = ?1",
            params![slug],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if is_builtin != 0 {
        return Err("Cannot delete a built-in agent.".to_string());
    }

    conn.execute("DELETE FROM agents WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
