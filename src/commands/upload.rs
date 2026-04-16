/// Upload a local file to tmpfiles.org and print the direct-download HTTPS URL.
///
/// Empirical findings (2026-04-16 session):
/// - catbox.moe / litterbox.catbox.moe URLs are blocklisted by BytePlus's fetcher.
///   Seedance returns `InvalidParameter: Invalid video_url` even though the URL
///   is reachable from outside.
/// - tmpfiles.org HTTPS direct-download URLs (`https://tmpfiles.org/dl/<id>/<name>`)
///   ARE accepted by the BytePlus fetcher. Content-Type must be correct (tmpfiles
///   sets `video/mp4` automatically for mp4 uploads).
/// - The BytePlus Files API (`/api/v3/files`) exists but file IDs are NOT valid
///   for `content[].video_url.url` — scheme validation rejects `file://<id>`.
///   Only `http(s)://` and `asset://` URIs pass.
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::AppError;
use crate::output::{self, Ctx};

const HOST: &str = "https://tmpfiles.org/api/v1/upload";

#[derive(Serialize)]
struct UploadResult {
    input: String,
    url: String,
    size_bytes: u64,
    host: &'static str,
}

pub fn run(ctx: Ctx, input: PathBuf) -> Result<(), AppError> {
    if !input.exists() {
        return Err(AppError::InvalidInput(format!(
            "file not found: {}",
            input.display()
        )));
    }
    let meta = std::fs::metadata(&input)?;
    output::info(
        ctx,
        &format!(
            "uploading {} ({} bytes) to tmpfiles.org",
            input.display(),
            meta.len()
        ),
    );

    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(AppError::from)?;
    let form = reqwest::blocking::multipart::Form::new()
        .file("file", &input)
        .map_err(|e| AppError::Transient(e.to_string()))?;

    let resp = http.post(HOST).multipart(form).send()?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Transient(format!(
            "tmpfiles upload failed: HTTP {status} body={body}"
        )));
    }

    // tmpfiles returns: {"status":"success","data":{"url":"http://tmpfiles.org/12345/file.mp4"}}
    // We need to rewrite to the direct-download URL: https://tmpfiles.org/dl/12345/file.mp4
    #[derive(serde::Deserialize)]
    struct Envelope {
        status: String,
        data: Option<Data>,
    }
    #[derive(serde::Deserialize)]
    struct Data {
        url: String,
    }
    let env: Envelope = serde_json::from_str(&body)
        .map_err(|e| AppError::Transient(format!("tmpfiles response not JSON: {e} -- {body}")))?;
    if env.status != "success" {
        return Err(AppError::Transient(format!(
            "tmpfiles returned status={} body={body}",
            env.status
        )));
    }
    let raw_url = env.data.map(|d| d.url).unwrap_or_default();
    // Rewrite http://tmpfiles.org/<id>/<file> -> https://tmpfiles.org/dl/<id>/<file>
    let direct_url = raw_url
        .replacen("http://tmpfiles.org/", "https://tmpfiles.org/dl/", 1)
        .replacen("https://tmpfiles.org/", "https://tmpfiles.org/dl/", 1)
        // Undo double-replacement if the source was already a /dl/ path
        .replace("/dl/dl/", "/dl/");

    let result = UploadResult {
        input: input.display().to_string(),
        url: direct_url.clone(),
        size_bytes: meta.len(),
        host: "tmpfiles.org",
    };
    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "url:".bold(), r.url.cyan());
        println!(
            "  pass to: seedance generate --video {} ...",
            r.url.as_str().dimmed()
        );
    });
    Ok(())
}
