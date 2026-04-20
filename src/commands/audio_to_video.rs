/// Wrap an audio file inside a silent-image mp4 so it can be passed to Seedance as a
/// `reference_video`. Workaround for Seedance 2.0's quirk: when you upload audio directly
/// via `--audio`, the model rewrites lyrics and melody. Feeding the same audio inside a
/// solid-color mp4 as a reference video preserves it verbatim with clean lip-sync.
/// (Credit: @simeonnz, amplified by @MrDavids1.)
///
/// **Hard duration cap of 14.5s.** The BytePlus docs say reference videos are capped at
/// 15s but empirically the API rejects anything >15.2s with
/// `InvalidParameter: video duration (seconds) must be <= 15.2`. Lavfi-based video can
/// end up slightly longer than the audio track due to frame-boundary alignment, so we
/// enforce `-t 14.5` to stay safely under.
///
/// Uses ffmpeg under the hood. Output is plain H.264 / AAC mp4 at 480p or 720p.
use serde::Serialize;
use std::path::PathBuf;

use crate::error::AppError;
use crate::output::{self, Ctx};

const HARD_DURATION_CAP_SECS: f64 = 14.5;

#[derive(Serialize)]
struct WrapResult {
    input: String,
    output: String,
    height: u32,
    background: String,
    duration_cap_secs: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    uploaded_url: Option<String>,
    hint: &'static str,
}

pub fn run(
    ctx: Ctx,
    input: PathBuf,
    output: Option<PathBuf>,
    background: String,
    height: u32,
    upload: bool,
) -> Result<(), AppError> {
    if !input.exists() {
        return Err(AppError::InvalidInput(format!(
            "audio file not found: {}",
            input.display()
        )));
    }
    if which::which("ffmpeg").is_err() {
        return Err(AppError::Config(
            "ffmpeg is not on PATH. Install with: brew install ffmpeg (macOS) / apt install ffmpeg (linux)".into(),
        ));
    }
    let bg = match background.to_ascii_lowercase().as_str() {
        "black" => "black",
        "white" => "white",
        other => {
            return Err(AppError::InvalidInput(format!(
                "--background must be 'black' or 'white'; got '{other}'"
            )));
        }
    };
    if !(height == 480 || height == 720) {
        return Err(AppError::InvalidInput(format!(
            "--height must be 480 or 720; got {height}"
        )));
    }

    let out_path = output.unwrap_or_else(|| default_output(&input));
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    // 864x480 is within BytePlus's pixel-count window [409600, 927408].
    // 1280x720 = 921600 also valid.
    let (w, h) = if height == 480 {
        (864, 480)
    } else {
        (1280, 720)
    };

    output::info(
        ctx,
        &format!(
            "wrapping {} -> {} ({}x{} {} bg, <= {} sec)",
            input.display(),
            out_path.display(),
            w,
            h,
            bg,
            HARD_DURATION_CAP_SECS
        ),
    );

    // ffmpeg:
    //   -f lavfi + color source = constant colour frames
    //   -t HARD_DURATION_CAP enforces the 14.5s ceiling BEFORE encoding so the
    //     resulting container is strictly within BytePlus's 15.2s real limit
    //     (discovered 2026-04-16 session — -shortest alone produced overruns).
    //   -tune stillimage optimises x264 for static frames (tiny file)
    //   -pix_fmt yuv420p for maximum player / API compatibility
    //   -r 24 to match Seedance's native frame rate
    let color_src = format!("color=c={bg}:s={w}x{h}:r=24");
    let dur = format!("{}", HARD_DURATION_CAP_SECS);
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        &color_src,
        "-i",
    ])
    .arg(&input)
    .args([
        "-t",
        &dur,
        "-c:v",
        "libx264",
        "-tune",
        "stillimage",
        "-preset",
        "veryfast",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
    ])
    .arg(&out_path);

    let status = cmd
        .status()
        .map_err(|e| AppError::Transient(format!("failed to spawn ffmpeg: {e}")))?;
    if !status.success() {
        return Err(AppError::Transient(format!(
            "ffmpeg exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }
    if !out_path.exists() {
        return Err(AppError::Transient(format!(
            "ffmpeg completed but {} was not created",
            out_path.display()
        )));
    }

    let uploaded_url = if upload {
        Some(upload_to_tmpfiles(&out_path)?)
    } else {
        None
    };

    let result = WrapResult {
        input: input.display().to_string(),
        output: out_path.display().to_string(),
        height,
        background: bg.into(),
        duration_cap_secs: HARD_DURATION_CAP_SECS,
        uploaded_url: uploaded_url.clone(),
        hint: if uploaded_url.is_some() {
            "URL is ready -- pass as --video <url>"
        } else {
            "host this mp4 publicly (tmpfiles.org / S3) and pass as --video <url>; or re-run with --upload"
        },
    };

    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "wrapped:".bold(), r.output.green());
        if let Some(u) = &r.uploaded_url {
            println!("{} {}", "url:".bold(), u.cyan());
            println!("next:  seedance generate --video {u} ...");
        } else {
            println!(
                "next:  upload publicly (e.g. `seedance upload {}`), pass as {}",
                r.output,
                "--video <url>".cyan()
            );
            println!(
                "note:  {}",
                "BytePlus rejects catbox.moe URLs; tmpfiles.org works".dimmed()
            );
        }
    });
    Ok(())
}

fn upload_to_tmpfiles(path: &std::path::Path) -> Result<String, AppError> {
    let http = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(AppError::from)?;
    let form = reqwest::blocking::multipart::Form::new()
        .file("file", path)
        .map_err(|e| AppError::Transient(e.to_string()))?;
    let resp = http
        .post("https://tmpfiles.org/api/v1/upload")
        .multipart(form)
        .send()?;
    let body = resp.text().unwrap_or_default();
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
    let raw = env.data.map(|d| d.url).unwrap_or_default();
    Ok(raw
        .replacen("http://tmpfiles.org/", "https://tmpfiles.org/dl/", 1)
        .replacen("https://tmpfiles.org/", "https://tmpfiles.org/dl/", 1)
        .replace("/dl/dl/", "/dl/"))
}

fn default_output(input: &std::path::Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    let parent = input.parent().unwrap_or(std::path::Path::new("."));
    parent.join(format!("{stem}.silent.mp4"))
}
