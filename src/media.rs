/// Convert a local file path or URL into an API-compatible string.
///
/// * URLs (http:// or https:// or asset://) are returned verbatim.
/// * Local paths are read and base64-encoded into a data: URL.
///
/// The BytePlus video generation API accepts `image_url.url` and `audio_url.url`
/// as either a real URL, an asset://<id>, or `data:<mime>;base64,<payload>`.
/// Video input is URL-only per the docs -- we reject local paths with a clear
/// InvalidInput.
use base64::Engine;
use std::path::Path;

use crate::error::AppError;

pub enum Kind {
    Image,
    Audio,
    Video,
}

const IMAGE_MAX_BYTES: u64 = 30 * 1024 * 1024;
const AUDIO_MAX_BYTES: u64 = 15 * 1024 * 1024;

fn is_url(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("asset://")
        || s.starts_with("data:")
}

/// Resolve a user-supplied reference (URL or path) to what the API wants in
/// `image_url.url` / `audio_url.url` / `video_url.url`.
pub fn resolve(reference: &str, kind: Kind) -> Result<String, AppError> {
    if is_url(reference) {
        return Ok(reference.to_string());
    }

    // Local path handling.
    let path = Path::new(reference);
    if !path.exists() {
        return Err(AppError::InvalidInput(format!(
            "not a URL and not an existing file: {reference}"
        )));
    }

    match kind {
        Kind::Video => Err(AppError::InvalidInput(format!(
            "video input requires a public URL -- local path not supported: {reference}. \
             Upload the mp4/mov to a CDN or pre-signed URL first."
        ))),
        Kind::Image => encode_to_data_url(path, IMAGE_MAX_BYTES, "image"),
        Kind::Audio => encode_to_data_url(path, AUDIO_MAX_BYTES, "audio"),
    }
}

fn encode_to_data_url(path: &Path, max_bytes: u64, prefix: &str) -> Result<String, AppError> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > max_bytes {
        return Err(AppError::InvalidInput(format!(
            "{} too large: {} bytes > {} bytes. Upload as a URL instead.",
            path.display(),
            meta.len(),
            max_bytes
        )));
    }
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_lowercase();
    if !mime.starts_with(prefix) {
        return Err(AppError::InvalidInput(format!(
            "{} does not look like a {prefix} file (mime: {mime})",
            path.display()
        )));
    }
    let bytes = std::fs::read(path)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}
