/// Wrap an audio file inside a silent-image mp4 so it can be passed to Seedance as a
/// `reference_video`. Workaround for Seedance 2.0's quirk: when you upload audio directly
/// via `--audio`, the model rewrites lyrics and melody. Feeding the same audio inside a
/// solid-color mp4 as a reference video preserves it verbatim with clean lip-sync.
/// (Credit: @simeonnz, amplified by @MrDavids1.)
///
/// Uses ffmpeg under the hood. Output is a plain H.264 / AAC mp4 at 480p or 720p.
use serde::Serialize;
use std::path::PathBuf;

use crate::error::AppError;
use crate::output::{self, Ctx};

#[derive(Serialize)]
struct WrapResult {
    input: String,
    output: String,
    height: u32,
    background: String,
    hint: &'static str,
}

pub fn run(
    ctx: Ctx,
    input: PathBuf,
    output: Option<PathBuf>,
    background: String,
    height: u32,
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

    // Seedance 2.0 accepts 480p or 720p reference videos, 9:16 / 16:9 / 1:1 all fine.
    // 480p at 16:9 = 854x480 but BytePlus table uses 864x496. We use 864x480 -- simple,
    // even dimensions, well within [409600, 927408] pixel product.
    let (w, h) = if height == 480 {
        (864, 480)
    } else {
        (1280, 720)
    };

    output::info(
        ctx,
        &format!(
            "wrapping {} -> {} ({}x{} {})",
            input.display(),
            out_path.display(),
            w,
            h,
            bg
        ),
    );

    // ffmpeg:
    //   -loop 1 on a lavfi color source = constant color frames
    //   -shortest ends the video when the audio ends
    //   -tune stillimage optimises x264 for static frames (tiny file)
    //   -pix_fmt yuv420p for maximum player / API compatibility
    //   -r 24 to match Seedance's native frame rate
    let color_src = format!("color=c={bg}:s={w}x{h}:r=24");
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args([
        "-y",           // overwrite
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
        "-shortest",
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

    let result = WrapResult {
        input: input.display().to_string(),
        output: out_path.display().to_string(),
        height,
        background: bg.into(),
        hint: "host this mp4 at a public URL, then pass as: seedance generate --video <url> ...",
    };

    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "wrapped:".bold(), r.output.green());
        println!(
            "next:  host the mp4 publicly (S3 / catbox.moe / signed URL) and pass it as {}",
            "--video <url>".cyan()
        );
        println!(
            "       Seedance API requires a real URL for video references -- {} is rejected",
            "base64 / local path".dimmed()
        );
    });
    Ok(())
}

fn default_output(input: &std::path::Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    let parent = input.parent().unwrap_or(std::path::Path::new("."));
    parent.join(format!("{stem}.silent.mp4"))
}
