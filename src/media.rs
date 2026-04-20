/// Convert a local file path or URL into an API-compatible string.
///
/// * `http://` / `https://` / `asset://` URLs are returned verbatim.
/// * `data:` URLs are validated per-kind (video rejected) and size-checked.
/// * Local paths are read and base64-encoded into a `data:<mime>;base64,...` URL.
///
/// The BytePlus video generation API accepts `image_url.url` and `audio_url.url`
/// as either a real URL, an `asset://<id>`, or `data:<mime>;base64,<payload>`.
/// Video input is URL-only per the docs -- we reject local paths AND inline
/// `data:` URLs for video to keep the CLI contract tight.
use base64::Engine;
use std::path::Path;

use crate::error::AppError;

pub enum Kind {
    Image,
    Audio,
    Video,
}

impl Kind {
    fn label(&self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Audio => "audio",
            Kind::Video => "video",
        }
    }
}

const IMAGE_MAX_BYTES: u64 = 30 * 1024 * 1024;
const AUDIO_MAX_BYTES: u64 = 15 * 1024 * 1024;

/// `data:<mime>;base64,` — the decoded bytes of the base64 payload are this ratio
/// of its encoded length. Used to estimate size without fully decoding.
const BASE64_RATIO: f64 = 0.75;

const AUDIO_ALLOWED_SUBTYPES: &[&str] = &["wav", "x-wav", "mpeg", "mp3"];

/// Resolve a user-supplied reference (URL or path) to what the API wants in
/// `image_url.url` / `audio_url.url` / `video_url.url`.
pub fn resolve(reference: &str, kind: Kind) -> Result<String, AppError> {
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return Ok(reference.to_string());
    }

    if reference.starts_with("asset://") {
        return Ok(reference.to_string());
    }

    if reference.starts_with("data:") {
        return validate_data_url(reference, &kind);
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
        Kind::Image => encode_to_data_url(path, IMAGE_MAX_BYTES, "image", None),
        Kind::Audio => {
            encode_to_data_url(path, AUDIO_MAX_BYTES, "audio", Some(AUDIO_ALLOWED_SUBTYPES))
        }
    }
}

fn validate_data_url(reference: &str, kind: &Kind) -> Result<String, AppError> {
    // data:<mime>;base64,<payload>
    let rest = reference
        .strip_prefix("data:")
        .ok_or_else(|| AppError::InvalidInput(format!("not a data URL: {reference}")))?;
    let (mime_and_enc, payload) = rest.split_once(',').ok_or_else(|| {
        AppError::InvalidInput(format!(
            "malformed data URL: missing comma separator ({reference})"
        ))
    })?;
    let parts: Vec<&str> = mime_and_enc.split(';').collect();
    let mime = parts
        .first()
        .copied()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let is_base64 = parts
        .iter()
        .any(|p| p.trim().eq_ignore_ascii_case("base64"));

    if mime.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "data URL is missing a MIME type ({kind})",
            kind = kind.label()
        )));
    }

    match kind {
        Kind::Video => {
            return Err(AppError::InvalidInput(format!(
                "video input requires a public URL -- data: URLs are not accepted for video ({mime})"
            )));
        }
        Kind::Image => {
            if !mime.starts_with("image/") {
                return Err(AppError::InvalidInput(format!(
                    "image input must have an image/* MIME type, got `{mime}`"
                )));
            }
        }
        Kind::Audio => {
            if !mime.starts_with("audio/") {
                return Err(AppError::InvalidInput(format!(
                    "audio input must have an audio/* MIME type, got `{mime}`"
                )));
            }
            let subtype = mime.split('/').nth(1).unwrap_or("");
            if !AUDIO_ALLOWED_SUBTYPES.contains(&subtype) {
                return Err(AppError::InvalidInput(format!(
                    "audio MIME `{mime}` not supported -- API accepts wav or mp3"
                )));
            }
        }
    }

    // Estimate decoded size without fully decoding -- reject oversize inputs.
    if is_base64 {
        let approx_bytes = (payload.len() as f64 * BASE64_RATIO) as u64;
        let max = match kind {
            Kind::Image => IMAGE_MAX_BYTES,
            Kind::Audio => AUDIO_MAX_BYTES,
            Kind::Video => unreachable!(),
        };
        if approx_bytes > max {
            return Err(AppError::InvalidInput(format!(
                "data URL payload is ~{approx_bytes} bytes, exceeds {kind} limit of {max} bytes",
                kind = kind.label()
            )));
        }
    }

    Ok(reference.to_string())
}

fn encode_to_data_url(
    path: &Path,
    max_bytes: u64,
    prefix: &str,
    allowed_subtypes: Option<&[&str]>,
) -> Result<String, AppError> {
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
    if let Some(allowed) = allowed_subtypes {
        let subtype = mime.split('/').nth(1).unwrap_or("");
        if !allowed.contains(&subtype) {
            return Err(AppError::InvalidInput(format!(
                "{} has MIME `{mime}` which is not accepted by the API -- allowed: {}",
                path.display(),
                allowed.join(", ")
            )));
        }
    }
    let bytes = std::fs::read(path)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}
