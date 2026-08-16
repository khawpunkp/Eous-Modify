use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::Path;

/// Reads an arbitrary local file (e.g. picked via the dialog plugin, which lives outside the
/// fs plugin's scoped capability) and returns it as a data: URL the webview can render directly
/// — avoids needing an asset-protocol scope entry for a user-chosen path.
#[tauri::command]
pub fn read_image_as_data_url(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;

    // Refused rather than sent as application/octet-stream, which the webview cannot render — so the
    // old behaviour was a blank preview with nothing to explain it.
    let extension =
        Path::new(&path).extension().and_then(|ext| ext.to_str()).unwrap_or_default();
    let mime = crate::image_format::mime_for_extension(extension).ok_or_else(|| {
        format!(
            "Eous doesn't support .{} images. Try one of: {}.",
            extension,
            crate::image_format::supported_extensions()
        )
    })?;

    Ok(format!("data:{};base64,{}", mime, STANDARD.encode(bytes)))
}
