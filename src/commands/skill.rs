use serde::Serialize;
use std::path::PathBuf;

use crate::error::AppError;
use crate::output::{self, Ctx};

fn skill_content() -> String {
    let name = env!("CARGO_PKG_NAME");
    format!(
        r#"---
name: {name}
description: >
  Generate video with ByteDance Seedance 2.0 from the terminal. Supports text-to-video,
  image-to-video (first / first+last / up to 9 reference images), reference videos,
  reference audio, and multimodal mixes. Run `{name} agent-info` for the full capability
  manifest, flags, and exit codes. Run `{name} doctor` before first use.
---

## {name}

A CLI wrapper for ByteDance Seedance 2.0 via BytePlus ModelArk. Binary is the tool --
run `{name} agent-info` for the machine-readable schema.

Fast path:
  {name} doctor
  {name} generate --prompt "A cat yawns at the camera" --wait --output cat.mp4

Key flags for `generate`:
  --prompt / -p           Text prompt (supports [Image N], [Video N], [Audio N], [0-4s])
  --image / -i            Reference image (repeatable, max 9; path or URL)
  --first-frame           First frame image (mode switch)
  --last-frame            Last frame image (requires --first-frame)
  --video / -v            Reference video URL (repeatable, max 3, URLs only)
  --audio / -a            Reference audio (repeatable, max 3; needs an image or video alongside)
  --duration / -d         Seconds [4,15] or -1 for auto
  --resolution / -r       480p | 720p (2.0 has no 1080p)
  --ratio                 16:9 | 4:3 | 1:1 | 3:4 | 9:16 | 21:9 | adaptive
  --fast                  Use Seedance 2.0 Fast
  --wait / --output       Block until done, download the mp4

Polling after an async generate:
  {name} status <id>
  {name} download <id> --output out.mp4

Auth: `SEEDANCE_API_KEY` or `ARK_API_KEY` env var. Get a key at https://console.byteplus.com/ark.
"#
    )
}

struct SkillTarget {
    name: &'static str,
    path: PathBuf,
}

fn home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn skill_targets() -> Vec<SkillTarget> {
    let h = home();
    let app = env!("CARGO_PKG_NAME");
    vec![
        SkillTarget {
            name: "Claude Code",
            path: h.join(format!(".claude/skills/{app}")),
        },
        SkillTarget {
            name: "Codex CLI",
            path: h.join(format!(".codex/skills/{app}")),
        },
        SkillTarget {
            name: "Gemini CLI",
            path: h.join(format!(".gemini/skills/{app}")),
        },
    ]
}

#[derive(Serialize)]
struct InstallResult {
    platform: String,
    path: String,
    status: String,
}

pub fn install(ctx: Ctx) -> Result<(), AppError> {
    let content = skill_content();
    let mut results: Vec<InstallResult> = Vec::new();
    for target in &skill_targets() {
        let skill_path = target.path.join("SKILL.md");
        if skill_path.exists()
            && std::fs::read_to_string(&skill_path).is_ok_and(|c| c == content)
        {
            results.push(InstallResult {
                platform: target.name.into(),
                path: skill_path.display().to_string(),
                status: "already_current".into(),
            });
            continue;
        }
        std::fs::create_dir_all(&target.path)?;
        std::fs::write(&skill_path, &content)?;
        results.push(InstallResult {
            platform: target.name.into(),
            path: skill_path.display().to_string(),
            status: "installed".into(),
        });
    }
    output::print_success_or(ctx, &results, |r| {
        use owo_colors::OwoColorize;
        for item in r {
            let marker = if item.status == "installed" { "+" } else { "=" };
            println!(
                " {} {} -> {}",
                marker.green(),
                item.platform.bold(),
                item.path.dimmed()
            );
        }
    });
    Ok(())
}

#[derive(Serialize)]
struct SkillStatus {
    platform: String,
    installed: bool,
    current: bool,
}

pub fn status(ctx: Ctx) -> Result<(), AppError> {
    let content = skill_content();
    let mut results: Vec<SkillStatus> = Vec::new();
    for target in &skill_targets() {
        let skill_path = target.path.join("SKILL.md");
        let (installed, current) = if skill_path.exists() {
            let current =
                std::fs::read_to_string(&skill_path).is_ok_and(|c| c == content);
            (true, current)
        } else {
            (false, false)
        };
        results.push(SkillStatus {
            platform: target.name.into(),
            installed,
            current,
        });
    }
    output::print_success_or(ctx, &results, |r| {
        use owo_colors::OwoColorize;
        let mut table = comfy_table::Table::new();
        table.set_header(vec!["Platform", "Installed", "Current"]);
        for item in r {
            table.add_row(vec![
                item.platform.clone(),
                if item.installed {
                    "Yes".green().to_string()
                } else {
                    "No".red().to_string()
                },
                if item.current {
                    "Yes".green().to_string()
                } else {
                    "No".dimmed().to_string()
                },
            ]);
        }
        println!("{table}");
    });
    Ok(())
}
