/// Build a 9-angle (or 4-angle) character reference sheet by shelling out to `nanaban`
/// (Nano Banana Pro / Gemini 3 Pro image model). The resulting PNG collage can be passed
/// to `seedance generate --image <sheet.png>` to keep a specific person consistent across
/// Seedance 2.0 shots without tripping the single-face-upload block.
///
/// Design credit: the "character-sheet grid as a single reference image" trick is the
/// canonical community workaround (@voxelplot Advanced Workflow #8, @wtry1102 origin).
use serde::Serialize;
use std::path::PathBuf;

use crate::error::AppError;
use crate::manifest;
use crate::output::{self, Ctx};

#[derive(Serialize)]
struct SheetResult {
    input: String,
    output: String,
    angles: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    character: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    model: &'static str,
    hint: String,
}

pub fn run(
    ctx: Ctx,
    input: String,
    output: Option<PathBuf>,
    style: Option<String>,
    angles: u8,
    character: Option<String>,
    project: Option<String>,
) -> Result<(), AppError> {
    if !(angles == 4 || angles == 9) {
        return Err(AppError::InvalidInput(format!(
            "--angles must be 4 (2x2) or 9 (3x3); got {angles}"
        )));
    }
    if which::which("nanaban").is_err() {
        return Err(AppError::Config(
            "nanaban is not on PATH. Install it with: npm i -g nanaban (or see https://github.com/paperfoot/nanaban-cli)".into(),
        ));
    }

    let char_slug = character.as_deref().and_then(manifest::slug);
    let project_slug = project.as_deref().and_then(manifest::slug);
    let out_path = output.unwrap_or_else(|| {
        default_sheet_path(&input, char_slug.as_deref(), project_slug.as_deref())
    });
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let prompt = build_prompt(angles, style.as_deref());

    output::info(
        ctx,
        &format!("generating {angles}-angle character sheet via nanaban (Nano Banana Pro)"),
    );

    let mut cmd = std::process::Command::new("nanaban");
    cmd.arg(&prompt)
        .arg("--pro")
        .arg("--ar")
        .arg("1:1")
        .arg("--size")
        .arg("2k")
        .arg("--ref")
        .arg(&input)
        .arg("-o")
        .arg(&out_path)
        .arg("--quiet");

    let status = cmd
        .status()
        .map_err(|e| AppError::Transient(format!("failed to spawn nanaban: {e}")))?;

    if !status.success() {
        return Err(AppError::Transient(format!(
            "nanaban exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    if !out_path.exists() {
        return Err(AppError::Transient(format!(
            "nanaban completed but no PNG was written to {}",
            out_path.display()
        )));
    }

    let hint = match &char_slug {
        Some(c) => format!(
            "reference [Image N] as '{c}' in the prompt. For multi-character scenes, build one sheet per character and pass each as a separate --image."
        ),
        None => "pass to seedance with: --image <path>. For multi-character scenes, build one sheet per character.".to_string(),
    };

    let result = SheetResult {
        input,
        output: out_path.display().to_string(),
        angles,
        character: char_slug,
        project: project_slug,
        model: "nano-banana-pro (gemini-3-pro-image-preview)",
        hint,
    };

    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "sheet:".bold(), r.output.green());
        if let Some(c) = &r.character {
            println!("name:  {c}");
        }
        println!(
            "next:  {} generate --image {} --prompt '...' --wait",
            "seedance".cyan(),
            r.output
        );
    });
    Ok(())
}

fn build_prompt(angles: u8, style: Option<&str>) -> String {
    let grid = if angles == 9 { "3x3" } else { "2x2" };
    let angles_list = if angles == 9 {
        "top-left: front headshot, neutral expression; \
         top-center: 3/4 right view; \
         top-right: right profile; \
         middle-left: 3/4 left view; \
         middle-center: front again but with a soft smile; \
         middle-right: left profile; \
         bottom-left: slight look-up; \
         bottom-center: slight look-down; \
         bottom-right: back-of-head view"
    } else {
        "top-left: front headshot; \
         top-right: 3/4 right view; \
         bottom-left: left profile; \
         bottom-right: 3/4 left with soft smile"
    };

    let mut prompt = format!(
        "Clean {grid} character reference sheet of the single person shown in the attached photo. \
         {angles} equal cells arranged as a {grid} grid, hairline-thin white dividers between cells. \
         Each cell: {angles_list}. \
         Identical person across every cell -- same face geometry, eye colour, skin tone, hair style and length, \
         and outfit. No text, no labels, no logos, no watermarks. \
         Clean studio three-point lighting, soft shadows, neutral pure white seamless backdrop, sharp focus, \
         4k editorial photography. \
         Headroom matched across cells. Neutral camera height, no tilt. Hands out of frame.",
        grid = grid,
        angles = angles,
        angles_list = angles_list,
    );
    if let Some(extra) = style
        && !extra.trim().is_empty()
    {
        prompt.push_str(" Additional style notes: ");
        prompt.push_str(extra.trim());
        prompt.push('.');
    }
    prompt
}

fn default_sheet_path(input: &str, character: Option<&str>, project: Option<&str>) -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = home.join("Documents").join("seedance");
    if let Some(p) = project {
        dir = dir.join(p);
    }
    let filename = match character {
        Some(c) => format!("{c}-sheet.png"),
        None => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            input.hash(&mut h);
            format!("character-sheet-{:08x}.png", h.finish())
        }
    };
    dir.join(filename)
}
