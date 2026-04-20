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
  manifest, flags, and exit codes. Run `{name} doctor` before first use. For prompt-writing
  guidance, decision trees, and use-case templates (UGC / marketing / cinematic), also
  install the companion `seedance-prompting` skill.
---

## {name}

A CLI wrapper for ByteDance Seedance 2.0 via BytePlus ModelArk. Binary is the tool --
run `{name} agent-info` for the machine-readable schema.

Fast path:
  {name} doctor
  {name} generate --prompt "A cat yawns at the camera" --wait

Key flags for `generate`:
  --prompt / -p           Text prompt (supports [Image N], [Video N], [Audio N], time codes)
  --image / -i            Reference image (repeatable, max 9; path or URL)
  --first-frame           First frame image (mode switch)
  --last-frame            Last frame image (requires --first-frame)
  --video / -v            Reference video URL (repeatable, max 3, URLs only)
  --audio / -a            Reference audio (repeatable, max 3; needs image or video alongside)
  --duration / -d         Seconds [4,15] or -1 for auto
  --resolution / -r       480p | 720p (2.0 has no 1080p)
  --ratio                 16:9 | 4:3 | 1:1 | 3:4 | 9:16 | 21:9 | adaptive
  --fast                  Use Seedance 2.0 Fast
  --wait / --output       Block until done, download mp4 to ~/Documents/seedance/ by default
  --label                 Human slug in the default filename + sidecar manifest (e.g. "cafe-opening")
  --project               Nest output under ~/Documents/seedance/<project>/

Every `--wait`ed generate writes a sidecar `<file>.seedance.json` beside the mp4
containing the full request (prompt, refs, model, seed, duration, task id, timestamps).
Agents: read the sidecar instead of guessing which prompt produced which file.
Example: `for f in *.seedance.json; do jq -r '[.downloaded_to, .label, .prompt] | @tsv' "$f"; done`.

Companion subcommands:
  {name} character-sheet <photo> --character NAME [--project NAME]
                                   9-angle reference grid that keeps one person consistent.
                                   Requires: nanaban (npm i -g nanaban).
                                   Multi-character scenes: run once per character with
                                   distinct --character names. Cap at 2 characters per
                                   shot (3+ breaks identity).
  {name} audio-to-video <audio>    Wrap audio in silent mp4 (preserves exact lyrics / music).
                                   Requires: ffmpeg (brew install ffmpeg).
  {name} prep-face <photo>         Heavy-grain recipe that passes ModelArk's face filter.
                                   Requires: imagemagick.

Async flow:
  {name} status <id>
  {name} download <id> --output out.mp4   # also writes <out>.seedance.json
  {name} cancel <id>

Setup + auth:
  {name} config set api-key ark-xxxxxxxx   # stored chmod 600, masked in `config show`
  # or: export SEEDANCE_API_KEY / ARK_API_KEY
  # get a key: https://console.byteplus.com/ark

Prompting principles (Seedance is prompt-sensitive, do not rely on logic):
  * Describe every visible element explicitly. Do not say "the lighting is appropriate";
    say "soft window light from camera-right, no ring light, slight motion blur".
  * Give every reference an explicit job. Unnamed refs are dropped silently.
    "[Image 1] for Alice's face and hair only, not her clothing. [Image 2] for the
     leather jacket." beats "use these references".
  * One verb per shot. Split multi-action shots into time-coded beats:
    "[0-4s]: Alice walks in; [4-9s]: Alice sits; [9-15s]: Alice smiles at Bob".
  * Negative prompts do not work. Rephrase positively. Not "no weird eyes", but
    "eyes natural, soft eye contact with the lens, blinks occasionally".
  * Early tokens dominate. Put camera language, subject, and style in the first half.
  * Prompt length 30-200 words. Too short = underspecified; too long = details ignored.
  * Plain literal language beats clever language.

Multi-character workflow:
  # One sheet per person, named
  {name} character-sheet alice.jpg --character alice --project cafe
  {name} character-sheet bob.jpg   --character bob   --project cafe

  # Both sheets as separate references, each with an explicit job
  {name} generate --project cafe --label cafe-opening \
    --image ~/Documents/seedance/cafe/alice-sheet.png \
    --image ~/Documents/seedance/cafe/bob-sheet.png \
    --prompt "[Image 1] is Alice's 9-angle reference. [Image 2] is Bob's 9-angle \
              reference. Keep Alice's face and hair matching [Image 1] exactly; \
              keep Bob's face and beard matching [Image 2] exactly. \
              [0-4s]: wide shot, Alice and Bob sit across a corner table in a sunlit \
              Parisian cafe, warm natural window light from camera-left. \
              [4-9s]: medium shot, Alice picks up her espresso cup with her right hand. \
              [9-14s]: close-up on Bob, he smiles gently, handheld tracking." \
    --duration 14 --resolution 720p --ratio 16:9 --wait

Output of the last command:
  ~/Documents/seedance/cafe/20260420T023015Z-cafe-opening-abc12345.mp4
  ~/Documents/seedance/cafe/20260420T023015Z-cafe-opening-abc12345.seedance.json

Consistent-person-only workflow (no second character):
  {name} character-sheet ./subject.jpg --character alice
  {name} generate --image ~/Documents/seedance/alice-sheet.png --label alice-walk \
    --prompt "[Image 1] is Alice's 9-angle reference. Her face and hair match [Image 1] \
              exactly. Medium tracking shot, Alice walks through a sunlit Parisian \
              cafe, handheld, warm natural window light." --duration 10 --wait

Exact music / dialogue (preserves lyrics verbatim):
  {name} audio-to-video song.mp3 --upload          # prints a public URL
  {name} generate --video <url> --image alice-sheet.png --label music-video \
    --prompt "Use [Video 1] as the soundtrack throughout, play the audio exactly as \
              provided. [Image 1] is Alice, keep her face matching [Image 1]. Alice \
              sings to camera, music-video aesthetic, 2.39:1." --duration 15 --wait

For deeper prompt-writing (UGC vs marketing vs cinematic templates, platform-specific
tips, decision trees for character consistency, word budgets), see the
`seedance-prompting` skill.
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
        if skill_path.exists() && std::fs::read_to_string(&skill_path).is_ok_and(|c| c == content) {
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
            let current = std::fs::read_to_string(&skill_path).is_ok_and(|c| c == content);
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
