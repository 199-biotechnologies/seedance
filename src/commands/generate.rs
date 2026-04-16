use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::api::{ApiClient, ContentItem, CreateTaskRequest, TaskInfo, UrlObject};
use crate::cli::GenerateArgs;
use crate::config::{self, DEFAULT_MODEL, DEFAULT_MODEL_FAST};
use crate::error::AppError;
use crate::media::{self, Kind};
use crate::output::{self, Ctx, Format};

#[derive(Serialize)]
struct GenerateResult {
    id: String,
    model: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_frame_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    downloaded_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<TaskInfo>,
}

pub fn run(ctx: Ctx, args: GenerateArgs) -> Result<(), AppError> {
    validate(&args)?;

    let cfg = config::load()?;
    let api_key = config::resolve_api_key(args.api_key.as_deref(), &cfg).ok_or_else(|| {
        AppError::Config(
            "no API key found. Set SEEDANCE_API_KEY (or ARK_API_KEY), pass --api-key, or write it to config."
                .into(),
        )
    })?;

    let model = args
        .model
        .clone()
        .unwrap_or_else(|| {
            if args.fast {
                DEFAULT_MODEL_FAST.to_string()
            } else if cfg.model == DEFAULT_MODEL {
                DEFAULT_MODEL.to_string()
            } else {
                cfg.model.clone()
            }
        });

    let content = build_content(&args)?;
    let generate_audio = if args.no_audio_sync { false } else { args.audio_sync };

    let request = CreateTaskRequest {
        model: model.clone(),
        content,
        resolution: Some(args.resolution.as_api().to_string()),
        ratio: Some(args.ratio.as_api().to_string()),
        duration: Some(args.duration),
        seed: Some(args.seed),
        generate_audio: Some(generate_audio),
        watermark: Some(args.watermark),
        callback_url: args.callback_url.clone(),
        safety_identifier: args.safety_identifier.clone(),
    };

    let api = ApiClient::new(&cfg.base_url, &api_key)?;
    output::info(ctx, &format!("creating task ({model})"));
    let created = api.create_task(&request)?;

    let should_wait = args.wait || args.output.is_some();
    if !should_wait {
        let result = GenerateResult {
            id: created.id.clone(),
            model,
            status: "queued".into(),
            video_url: None,
            last_frame_url: None,
            downloaded_to: None,
            task: None,
        };
        output::print_success_or(ctx, &result, |r| {
            use owo_colors::OwoColorize;
            println!("{} {}", "task id:".bold(), r.id.cyan());
            println!("model: {}", r.model);
            println!(
                "poll with: {} status {}",
                "seedance".green(),
                r.id.cyan()
            );
        });
        return Ok(());
    }

    let task = wait_for_task(ctx, &api, &created.id, args.poll_interval, args.timeout)?;

    if task.status != "succeeded" {
        let msg = task
            .error
            .as_ref()
            .and_then(|e| e.message.clone())
            .unwrap_or_else(|| format!("task ended with status: {}", task.status));
        return Err(AppError::Api {
            code: task
                .error
                .as_ref()
                .and_then(|e| e.code.clone())
                .unwrap_or_else(|| task.status.clone()),
            message: msg,
        });
    }

    let video_url = task
        .video_url()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Transient("task succeeded but returned no video_url".into()))?;

    // --wait alone (no explicit --output) still writes the file -- default to
    // ~/Documents/seedance/<id>.mp4 so generations always land somewhere
    // predictable instead of scattering across whichever directory invoked us.
    let out_path = args
        .output
        .clone()
        .map(|p| normalize_output_path(p, &created.id))
        .unwrap_or_else(|| default_output_dir().join(format!("{}.mp4", created.id)));
    output::info(ctx, &format!("downloading to {}", out_path.display()));
    let bytes = api.download_video(&video_url, &out_path)?;
    output::info(ctx, &format!("wrote {bytes} bytes"));
    let downloaded_to = Some(out_path.display().to_string());

    let result = GenerateResult {
        id: task.id.clone(),
        model: task.model.clone().unwrap_or(model),
        status: task.status.clone(),
        video_url: Some(video_url),
        last_frame_url: task
            .content
            .as_ref()
            .and_then(|c| c.last_frame_url.clone()),
        downloaded_to,
        task: Some(task),
    };

    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "status:".bold(), r.status.green());
        println!("id:    {}", r.id);
        if let Some(u) = &r.video_url {
            println!("video: {}", u.cyan());
        }
        if let Some(p) = &r.downloaded_to {
            println!("saved: {}", p.green());
        }
    });
    Ok(())
}

fn validate(args: &GenerateArgs) -> Result<(), AppError> {
    // Must have something to generate from.
    let has_prompt = args.prompt.as_deref().is_some_and(|s| !s.trim().is_empty());
    let has_refs = !args.images.is_empty()
        || !args.videos.is_empty()
        || !args.audio.is_empty()
        || args.first_frame.is_some();
    if !has_prompt && !has_refs {
        return Err(AppError::InvalidInput(
            "provide at least --prompt or one reference (--image / --first-frame / --video / --audio)".into(),
        ));
    }

    if args.images.len() > 9 {
        return Err(AppError::InvalidInput(format!(
            "too many reference images: {}. Max 9 for Seedance 2.0.",
            args.images.len()
        )));
    }
    if args.videos.len() > 3 {
        return Err(AppError::InvalidInput(format!(
            "too many reference videos: {}. Max 3 (and total duration <=15s).",
            args.videos.len()
        )));
    }
    if args.audio.len() > 3 {
        return Err(AppError::InvalidInput(format!(
            "too many reference audio clips: {}. Max 3 (and total duration <=15s).",
            args.audio.len()
        )));
    }

    if !args.audio.is_empty()
        && args.images.is_empty()
        && args.videos.is_empty()
        && args.first_frame.is_none()
    {
        return Err(AppError::InvalidInput(
            "audio cannot be the only reference -- add at least one --image or --video".into(),
        ));
    }

    if args.duration != -1 && !(4..=15).contains(&args.duration) {
        return Err(AppError::InvalidInput(format!(
            "duration {} out of range. Use [4,15] or -1 for auto.",
            args.duration
        )));
    }

    if args.audio_sync && args.no_audio_sync {
        return Err(AppError::InvalidInput(
            "--audio-sync and --no-audio-sync are mutually exclusive".into(),
        ));
    }

    Ok(())
}

fn build_content(args: &GenerateArgs) -> Result<Vec<ContentItem>, AppError> {
    let mut items: Vec<ContentItem> = Vec::new();

    if let Some(prompt) = args.prompt.as_deref().map(str::trim)
        && !prompt.is_empty()
    {
        items.push(ContentItem::Text {
            text: prompt.to_string(),
        });
    }

    if let Some(first) = &args.first_frame {
        items.push(ContentItem::ImageUrl {
            image_url: UrlObject {
                url: media::resolve(first, Kind::Image)?,
            },
            role: Some("first_frame".into()),
        });
    }
    if let Some(last) = &args.last_frame {
        items.push(ContentItem::ImageUrl {
            image_url: UrlObject {
                url: media::resolve(last, Kind::Image)?,
            },
            role: Some("last_frame".into()),
        });
    }
    for img in &args.images {
        items.push(ContentItem::ImageUrl {
            image_url: UrlObject {
                url: media::resolve(img, Kind::Image)?,
            },
            role: Some("reference_image".into()),
        });
    }
    for vid in &args.videos {
        items.push(ContentItem::VideoUrl {
            video_url: UrlObject {
                url: media::resolve(vid, Kind::Video)?,
            },
            role: Some("reference_video".into()),
        });
    }
    for aud in &args.audio {
        items.push(ContentItem::AudioUrl {
            audio_url: UrlObject {
                url: media::resolve(aud, Kind::Audio)?,
            },
            role: Some("reference_audio".into()),
        });
    }

    Ok(items)
}

fn wait_for_task(
    ctx: Ctx,
    api: &ApiClient,
    id: &str,
    poll_interval: u64,
    timeout: u64,
) -> Result<TaskInfo, AppError> {
    let start = Instant::now();
    let interval = Duration::from_secs(poll_interval.max(1));
    let deadline = if timeout == 0 {
        None
    } else {
        Some(start + Duration::from_secs(timeout))
    };

    // Progress bar in human mode only.
    let bar = if matches!(ctx.format, Format::Human) && !ctx.quiet {
        let b = ProgressBar::new_spinner();
        b.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        b.enable_steady_tick(Duration::from_millis(120));
        Some(b)
    } else {
        None
    };

    loop {
        let task = api.get_task(id)?;
        if let Some(b) = &bar {
            let elapsed = start.elapsed().as_secs();
            b.set_message(format!(
                "{} ({elapsed}s) -- {}",
                id,
                task.status
            ));
        }
        if task.is_terminal() {
            if let Some(b) = bar {
                b.finish_and_clear();
            }
            return Ok(task);
        }
        if let Some(d) = deadline
            && Instant::now() >= d
        {
            if let Some(b) = bar {
                b.finish_and_clear();
            }
            return Err(AppError::Transient(format!(
                "timed out after {timeout}s waiting for task {id} (last status: {})",
                task.status
            )));
        }
        std::thread::sleep(interval);
    }
}

fn normalize_output_path(path: PathBuf, id: &str) -> PathBuf {
    if path.is_dir() || path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
        path.join(format!("{id}.mp4"))
    } else {
        path
    }
}

/// Default output directory when --wait is set without --output.
/// Resolves to ~/Documents/seedance/ so generated videos don't scatter around
/// wherever the user happened to invoke the CLI.
pub fn default_output_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join("Documents").join("seedance")
}
