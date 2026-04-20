<div align="center">

# Seedance CLI — ByteDance Seedance 2.0 from Your Terminal

**Generate AI video with text, images, and reference footage. One Rust binary. No MCP server required.**

<br />

[![Star this repo](https://img.shields.io/github/stars/paperfoot/seedance-cli?style=for-the-badge&logo=github&label=%E2%AD%90%20Star%20this%20repo&color=yellow)](https://github.com/paperfoot/seedance-cli/stargazers)
&nbsp;&nbsp;
[![Follow @longevityboris](https://img.shields.io/badge/Follow_%40longevityboris-000000?style=for-the-badge&logo=x&logoColor=white)](https://x.com/longevityboris)

<br />

[![Crates.io](https://img.shields.io/crates/v/seedance?style=for-the-badge&logo=rust&color=orange)](https://crates.io/crates/seedance)
&nbsp;
[![CI](https://img.shields.io/github/actions/workflow/status/paperfoot/seedance-cli/ci.yml?branch=main&style=for-the-badge&logo=github&label=CI)](https://github.com/paperfoot/seedance-cli/actions)
&nbsp;
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)
&nbsp;
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?style=for-the-badge)](CONTRIBUTING.md)

---

Pay-per-second video generation from ByteDance, wrapped in a tool agents can actually drive. JSON on pipe, human-readable on TTY, semantic exit codes, one static binary.

[Install](#install) · [API Key Setup](#api-key-setup) · [Quick Start](#quick-start) · [Reference Inputs](#reference-inputs) · [Companion Tools](#companion-tools) · [Known Quirks](#known-quirks) · [Commands](#commands)

</div>

---

## Why this exists

Seedance 2.0 produces strong 480p and 720p output at a low per-second price. The ModelArk API that serves it wasn't built for scripts — auth is awkward, responses are verbose, and there is no single step from prompt to mp4 on disk. This CLI gives you that step, and keeps the output shape stable whether you're piping into `jq` or watching a progress bar.

## Install

```bash
# Cargo (works everywhere Rust does)
cargo install seedance

# Homebrew (macOS + Linux)
brew install 199-biotechnologies/tap/seedance

# Prebuilt binary
# https://github.com/paperfoot/seedance-cli/releases/latest
```

## API Key Setup

BytePlus is ByteDance's official international cloud. The model platform is ModelArk. Three minutes, start to finish:

1. Go to **[console.byteplus.com/ark](https://console.byteplus.com/ark)** and sign in (free account, credit card needed for paid generation).
2. Open the **API Keys** page from the left sidebar.
3. Click **Create API Key** — copy the `ark-…` string it shows you (it won't show again).
4. Hand it to the CLI in one of three ways, pick whatever fits:

```bash
# (a) Environment variable — best for CI
export SEEDANCE_API_KEY=ark-xxxxxxxx
# ARK_API_KEY is also accepted

# (b) Config file — stored locally at chmod 600, never echoed back
seedance config set api-key ark-xxxxxxxx

# (c) Per-command flag — overrides everything else
seedance generate --prompt "…" --api-key ark-xxxxxxxx
```

Verify:

```bash
seedance doctor
```

If anything's wrong, `doctor` tells you which step to revisit.

## Quick Start

```bash
seedance generate --prompt "A cat yawns at the camera" --wait
# -> ~/Documents/seedance/<task-id>.mp4
```

Default output is `~/Documents/seedance/`. Override with `-o /path/to/file.mp4` or `-o /some/dir/`.

## How it works

```
 prompt + refs          ModelArk queue            mp4
     |                        |                    ^
     v                        v                    |
  seedance gen   ->    task id returned   ->   seedance download
                              |
                              v
                       seedance status <id>
```

One task id from start to finish. Fire and forget, or pass `--wait` to block until the file lands on disk.

## Reference Inputs

Seedance 2.0 accepts a free mix of references in a single `content` array:

| Flag            | Kind  | Limit | Notes |
|-----------------|-------|-------|-------|
| `--image / -i`  | Image | 0–9   | Path (base64'd inline) or URL. Role: `reference_image` |
| `--first-frame` | Image | 1     | Role: `first_frame` |
| `--last-frame`  | Image | 1     | Role: `last_frame` (requires `--first-frame`) |
| `--video / -v`  | Video | 0–3   | **URL only** (API restriction). Total ≤ 15s |
| `--audio / -a`  | Audio | 0–3   | wav/mp3. Path (base64'd) or URL. Total ≤ 15s. Needs an image or video alongside. |

Address references inside the prompt:

```
[Image 1] the boy waves; [Video 1] camera style; [Audio 1] background music
```

Use time codes for multi-shot: `[0-4s]: wide establishing shot; [4-8s]: push in`.

Supported resolutions: `480p`, `720p`. Supported ratios: `16:9`, `4:3`, `1:1`, `3:4`, `9:16`, `21:9`, `adaptive`. Duration: 4–15s or `-1` for auto.

## Examples

```bash
# Text-to-video, wait and download
seedance generate \
  --prompt "A kitten yawns and blinks at the camera, cozy warm light" \
  --duration 6 --resolution 720p --ratio 16:9 \
  --wait --output kitten.mp4

# Multimodal reference-to-video with the fast tier
seedance generate \
  --prompt "[Image 1] the boy smiles; [Image 2] the corgi jumps in; [Video 1] camera motion" \
  --image boy.png --image corgi.png \
  --video https://my-cdn.example/style.mp4 \
  --fast --wait -o out.mp4

# Fire and forget, poll later
TASK=$(seedance gen --prompt "..." | jq -r '.data.id')
seedance status "$TASK"
seedance download "$TASK" -o final.mp4
```

## Companion Tools

Four helpers ship in the same binary, built to work around specific Seedance limits. Each one chains cleanly into `generate`.

| Tool                         | What it does |
|------------------------------|--------------|
| `seedance character-sheet`   | Builds a 9-angle (or 4-angle) character sheet from a single photo via Nano Banana Pro. Feed the resulting PNG to `--image` to keep one person consistent across shots — bypasses the single-face-upload block. |
| `seedance audio-to-video`    | Wraps an audio file in a silent mp4 (ffmpeg under the hood) so you can pass it as `--video` instead of `--audio`. Preserves lyrics exactly. `--upload` hosts the file on tmpfiles.org and prints the URL. |
| `seedance prep-face`         | Applies an empirically verified grain + desaturation recipe so a real portrait clears ModelArk's face filter. `--bw` swaps to the pure grayscale variant. |
| `seedance upload <file>`     | Uploads a local file to tmpfiles.org and prints a direct-download URL, ready to paste into `--video`. |

## Known Quirks

Hard-won notes from real users, saved so you don't hit the same walls:

- **Audio upload mutates lyrics.** Reported by @MrDavids1 and @simeonnz: uploading audio directly alters the song. Workaround — `seedance audio-to-video song.mp3 --upload`, then pass the printed URL as `--video`. The API trusts reference videos for audio content but post-processes raw audio.
- **No real human faces in references.** ModelArk blocks direct upload of real human portraits. Either run the photo through `seedance prep-face` first, or build a `character-sheet` from one photo and use that.
- **Videos must be URLs.** Host the file first (S3, Cloudinary, or `seedance upload` → tmpfiles.org) and pass the URL. Local files are rejected by the API.
- **No 1080p.** Seedance 2.0 tops out at 720p. Use `--resolution 720p` — `1080p` returns an error.

## Commands

| Command | Purpose |
|---------|---------|
| `seedance generate` / `gen` | Create a video generation task |
| `seedance status <id>` / `get` | Poll a task |
| `seedance download <id>` | Download the mp4 for a completed task |
| `seedance cancel <id>` / `rm` | Cancel a queued task |
| `seedance character-sheet <photo>` | Build a consistent-character reference grid |
| `seedance audio-to-video <file>` | Wrap audio in silent mp4 (lyrics workaround) |
| `seedance prep-face <photo>` | Prepare a portrait to pass the face filter |
| `seedance upload <file>` | Host a local file and print a public URL |
| `seedance models` / `ls` | List available model ids |
| `seedance doctor` | Check API key, base URL, and deps |
| `seedance agent-info` / `info` | Machine-readable capability manifest |
| `seedance skill install` | Deploy SKILL.md to Claude, Codex, or Gemini |
| `seedance config show / path / set / unset` | Manage the TOML config |
| `seedance update [--check]` | Self-update from GitHub Releases |

Global flags: `--json` (force JSON), `--quiet` (suppress info), `--help`, `--version`.

## Exit codes

| Code | Meaning |
|------|---------|
| `0`  | Success |
| `1`  | Transient (network, API, IO, update) — retry |
| `2`  | Config (missing API key, bad base URL) — fix setup |
| `3`  | Invalid input — fix arguments |
| `4`  | Rate limited — wait and retry |

Every error also prints a machine-readable `error_code` and a literal recovery suggestion when you're in `--json` mode.

## Built with

- [agent-cli-framework](https://github.com/paperfoot/agent-cli-framework) — the scaffolding every paperfoot CLI is built on.

## Contributing

PRs and issues welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the short version.

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Built by [Boris Djordjevic](https://github.com/longevityboris) at [Paperfoot AI](https://paperfoot.com)

<br />

**If this saved you time:**

[![Star this repo](https://img.shields.io/github/stars/paperfoot/seedance-cli?style=for-the-badge&logo=github&label=%E2%AD%90%20Star%20this%20repo&color=yellow)](https://github.com/paperfoot/seedance-cli/stargazers)
&nbsp;&nbsp;
[![Follow @longevityboris](https://img.shields.io/badge/Follow_%40longevityboris-000000?style=for-the-badge&logo=x&logoColor=white)](https://x.com/longevityboris)

</div>
