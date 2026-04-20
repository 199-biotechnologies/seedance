use serde::Serialize;
use std::path::PathBuf;

use crate::api::ApiClient;
use crate::config;
use crate::error::AppError;
use crate::manifest::{self, Manifest, References};
use crate::output::{self, Ctx};

#[derive(Serialize)]
struct DownloadResult {
    id: String,
    path: String,
    bytes: u64,
    video_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest: Option<String>,
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

    // Reconstruct a manifest from whatever the API echoed back. We won't have
    // the original prompt here (the API doesn't return the request payload),
    // so `prompt` is left None. The manifest is still useful as a trail:
    // task id, model, resolution, seed, duration, timestamps.
    let last_frame_url = task.content.as_ref().and_then(|c| c.last_frame_url.clone());
    let m = Manifest {
        schema: "seedance.v1",
        task_id: task.id.clone(),
        model: task.model.clone().unwrap_or_default(),
        status: task.status.clone(),
        created_at: task
            .created_at
            .map(manifest::iso8601_from_epoch_secs)
            .unwrap_or_else(manifest::iso8601_now),
        label: None,
        project: None,
        prompt: None,
        resolution: task.resolution.clone(),
        ratio: task.ratio.clone(),
        duration: task.duration,
        seed: task.seed,
        generate_audio: task.generate_audio,
        references: References::default(),
        video_url: Some(video_url.clone()),
        last_frame_url,
        downloaded_to: path.display().to_string(),
    };
    let manifest_path = match manifest::write(&path, &m) {
        Ok(p) => Some(p.display().to_string()),
        Err(e) => {
            output::info(ctx, &format!("warning: manifest write failed: {e}"));
            None
        }
    };

    let result = DownloadResult {
        id,
        path: path.display().to_string(),
        bytes,
        video_url,
        manifest: manifest_path,
    };
    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "saved:".bold(), r.path.green());
        println!("bytes: {}", r.bytes);
        if let Some(m) = &r.manifest {
            println!("meta:  {}", m.dimmed());
        }
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
