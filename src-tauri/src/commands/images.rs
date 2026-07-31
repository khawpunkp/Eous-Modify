use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::Path;

/// Reads an arbitrary local file (e.g. picked via the dialog plugin, which lives outside the
/// fs plugin's scoped capability) and returns it as a data: URL the webview can render directly
/// — avoids needing an asset-protocol scope entry for a user-chosen path.
#[tauri::command]
pub fn read_image_as_data_url(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;

    let mime = match Path::new(&path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    };

    Ok(format!("data:{};base64,{}", mime, STANDARD.encode(bytes)))
}
