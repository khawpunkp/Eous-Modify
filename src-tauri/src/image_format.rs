//! The one list of image formats this app handles, in both directions.
//!
//! Two places used to decide this independently and disagree: reading a file off disk mapped an
//! unknown extension to `application/octet-stream`, while decoding a `data:` URL mapped an unknown
//! type to `png`. That second default is the dangerous one — it wrote whatever bytes it had been
//! given into a file called `mod_preview.png`, so a pasted AVIF became a PNG that is not a PNG, and
//! nothing could display it afterwards. Neither reported anything.
//!
//! So both directions read the table below, and both refuse what is not in it. An unsupported format
//! should fail where the user can still choose a different file, not three steps later as an image
//! that silently will not render.

/// `(extension, mime)`, extensions lowercase and without the dot.
///
/// Several extensions map to one type on purpose — `jpe`/`jif`/`jfif` are all JPEG, and file
/// managers do still produce them. The reverse direction picks the canonical one below.
const FORMATS: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("apng", "image/apng"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("jpe", "image/jpeg"),
    ("jif", "image/jpeg"),
    ("jfif", "image/jpeg"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
    ("svg", "image/svg+xml"),
    ("ico", "image/x-icon"),
    ("bmp", "image/bmp"),
    ("avif", "image/avif"),
    ("heic", "image/heic"),
    ("heif", "image/heif"),
    ("jxl", "image/jxl"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
];

/// The extension to save under, for types with more than one spelling.
///
/// APNG deliberately lands on `png`: the file really is a PNG, every APNG in the wild carries that
/// extension, and writing `.apng` would only make it less likely to open elsewhere.
const CANONICAL: &[(&str, &str)] = &[
    ("image/jpeg", "jpg"),
    ("image/apng", "png"),
    ("image/tiff", "tiff"),
    ("image/vnd.microsoft.icon", "ico"),
];

/// The MIME type for a file extension, or `None` if this app does not handle it.
pub fn mime_for_extension(extension: &str) -> Option<&'static str> {
    let lowered = extension.to_lowercase();
    FORMATS.iter().find(|(ext, _)| *ext == lowered).map(|(_, mime)| *mime)
}

/// The extension to save a given MIME type under, or `None` if this app does not handle it.
pub fn extension_for_mime(mime: &str) -> Option<&'static str> {
    let lowered = mime.trim().to_lowercase();

    if let Some((_, ext)) = CANONICAL.iter().find(|(m, _)| *m == lowered) {
        return Some(ext);
    }
    FORMATS.iter().find(|(_, m)| *m == lowered).map(|(ext, _)| *ext)
}

/// Every extension handled, for messages that need to say what is accepted.
pub fn supported_extensions() -> String {
    let mut names: Vec<&str> = FORMATS.iter().map(|(ext, _)| *ext).collect();
    names.sort_unstable();
    names.dedup();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_both_directions_for_a_simple_format() {
        assert_eq!(mime_for_extension("webp"), Some("image/webp"));
        assert_eq!(extension_for_mime("image/webp"), Some("webp"));
    }

    #[test]
    fn every_jpeg_spelling_reads_as_jpeg_and_writes_as_jpg() {
        for ext in ["jpg", "jpeg", "jpe", "jif", "jfif"] {
            assert_eq!(mime_for_extension(ext), Some("image/jpeg"), "{ext}");
        }
        assert_eq!(extension_for_mime("image/jpeg"), Some("jpg"));
    }

    #[test]
    fn apng_is_saved_as_png() {
        assert_eq!(extension_for_mime("image/apng"), Some("png"));
    }

    #[test]
    fn extensions_and_types_are_matched_case_insensitively() {
        assert_eq!(mime_for_extension("PNG"), Some("image/png"));
        assert_eq!(extension_for_mime("IMAGE/PNG"), Some("png"));
    }

    /// The whole point of the module: unknown means no, not a silent guess at png.
    #[test]
    fn refuses_what_it_does_not_know() {
        assert_eq!(mime_for_extension("psd"), None);
        assert_eq!(extension_for_mime("image/vnd.adobe.photoshop"), None);
        assert_eq!(extension_for_mime("application/octet-stream"), None);
    }

    /// Every type in the table can be written back out, or a file could be read and then refused on
    /// the way to disk.
    #[test]
    fn every_readable_format_is_also_writable() {
        for (ext, mime) in FORMATS {
            assert!(
                extension_for_mime(mime).is_some(),
                "{mime} (from .{ext}) has no extension to save under",
            );
        }
    }
}
