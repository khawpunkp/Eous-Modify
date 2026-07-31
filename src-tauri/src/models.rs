use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWithAliases {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub details: Option<String>,
    /// Already resolved: the custom image if one is set, otherwise the seeded one.
    pub base_image: Option<String>,
    /// Whether `base_image` came from a user pick — drives the "use default" affordance.
    pub has_custom_image: bool,
    pub is_builtin: bool,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInput {
    pub name: String,
    pub details: Option<String>,
    pub base_image: Option<String>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModWithState {
    pub id: i64,
    pub agent_id: Option<i64>,
    pub category_id: Option<i64>,
    pub category_item_id: Option<i64>,
    pub name: String,
    pub folder_name: String,
    pub image_filename: Option<String>,
    pub author: Option<String>,
    pub is_enabled: bool,
    pub group_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInput {
    pub name: String,
    pub author: Option<String>,
    /// A freshly-picked preview image as a data: URL, if the user chose a new one — saved to disk
    /// inside the mod's own folder (keeping image_filename a plain relative filename, same
    /// convention the scanner already uses), not stored as a data URL in the DB like agent images.
    pub image_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KeybindInfo {
    pub title: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupMember {
    pub mod_id: i64,
    pub name: String,
    pub folder_name: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupWithMembers {
    pub id: i64,
    pub name: String,
    pub base_image: Option<String>,
    pub is_enabled: bool,
    pub members: Vec<ModGroupMember>,
}
