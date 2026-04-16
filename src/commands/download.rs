use serde::Serialize;
use std::path::PathBuf;

use crate::api::ApiClient;
use crate::config;
use crate::error::AppError;
use crate::output::{self, Ctx};

#[derive(Serialize)]
struct DownloadResult {
    id: String,
    path: String,
    bytes: u64,
    video_url: String,
}

pub fn run(
    ctx: Ctx,
    id: String,
    output_path: Option<PathBuf>,
    api_key: Option<String>,
) -> Result<(), AppError> {
    let cfg = config::load()?;
    let key = config::resolve_api_key(api_key.as_deref(), &cfg).ok_or_else(|| {
        AppError::Config(
            "no API key found. Set SEEDANCE_API_KEY (or ARK_API_KEY) or pass --api-key.".into(),
        )
    })?;
    let api = ApiClient::new(&cfg.base_url, &key)?;

    let task = api.get_task(&id)?;
    if task.status != "succeeded" {
        return Err(AppError::InvalidInput(format!(
            "task {id} is not ready for download (status: {}). Poll `seedance status {id}` first.",
            task.status
        )));
    }
    let video_url = task
        .video_url()
        .ok_or_else(|| AppError::Transient("task succeeded but returned no video_url".into()))?
        .to_string();

    let path = resolve_path(output_path, &id);
    output::info(ctx, &format!("downloading to {}", path.display()));
    let bytes = api.download_video(&video_url, &path)?;

    let result = DownloadResult {
        id,
        path: path.display().to_string(),
        bytes,
        video_url,
    };
    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "saved:".bold(), r.path.green());
        println!("bytes: {}", r.bytes);
    });
    Ok(())
}

fn resolve_path(provided: Option<PathBuf>, id: &str) -> PathBuf {
    match provided {
        Some(p) if p.is_dir() || p.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) => {
            p.join(format!("{id}.mp4"))
        }
        Some(p) => p,
        None => crate::commands::generate::default_output_dir().join(format!("{id}.mp4")),
    }
}
