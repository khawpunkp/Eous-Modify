pub const SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS categories (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    slug TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS agents (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    slug        TEXT UNIQUE NOT NULL,
    details     TEXT,
    base_image  TEXT,
    -- A user-picked image. Kept separate from base_image so definition re-sync (which always
    -- rewrites base_image) can't clobber it; reads COALESCE the two, so NULL means "use the seeded
    -- image". Clearing it is how "use default" works.
    custom_image TEXT,
    is_builtin  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS agent_aliases (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id INTEGER NOT NULL,
    alias    TEXT NOT NULL,
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE,
    UNIQUE (agent_id, alias)
);

CREATE TABLE IF NOT EXISTS category_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    name        TEXT NOT NULL,
    slug        TEXT UNIQUE NOT NULL,
    description TEXT,
    details     TEXT,
    base_image  TEXT,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS mods (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id          INTEGER,   -- nullable: UI/uncategorized mods
    category_id       INTEGER,
    category_item_id  INTEGER,
    name              TEXT NOT NULL,
    folder_name       TEXT NOT NULL UNIQUE,
    image_filename    TEXT,
    author            TEXT,
    FOREIGN KEY (agent_id)         REFERENCES agents(id)         ON DELETE SET NULL,
    FOREIGN KEY (category_id)      REFERENCES categories(id)     ON DELETE SET NULL,
    FOREIGN KEY (category_item_id) REFERENCES category_items(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS mod_groups (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    -- A data: URL, same convention agents' base_image uses for user-picked images. Groups have
    -- no folder of their own on disk, so there's nowhere to put a plain filename.
    base_image TEXT
);

CREATE TABLE IF NOT EXISTS mod_group_members (
    group_id INTEGER NOT NULL,
    mod_id   INTEGER NOT NULL UNIQUE, -- a mod belongs to at most one group
    PRIMARY KEY (group_id, mod_id),
    FOREIGN KEY (group_id) REFERENCES mod_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (mod_id)   REFERENCES mods(id)        ON DELETE CASCADE
);

-- Persistent 3DMigoto variables ($swapvar and friends) parked here while a mod is disabled.
--
-- 3DMigoto keys these by the mod's ini path, so the DISABLED_ rename changes the key and it discards
-- the entry from d3dx_user.ini as unrecognised. Snapshotting on disable and writing the values back
-- on enable is what keeps a mod's in-game toggle choices across an off/on cycle.
CREATE TABLE IF NOT EXISTS mod_persisted_vars (
    mod_id  INTEGER NOT NULL,
    -- Full d3dx_user.ini key, e.g. `$\mods\robot aria\ariahumanzzz.ini\facea2`.
    var_key TEXT NOT NULL,
    value   TEXT NOT NULL,
    PRIMARY KEY (mod_id, var_key),
    FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
"#;
