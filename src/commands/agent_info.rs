/// Machine-readable capability manifest.
pub fn run() {
    let name = env!("CARGO_PKG_NAME");
    let config_path = crate::config::config_path();

    let info = serde_json::json!({
        "name": name,
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "commands": {
            "generate": {
                "description": "Create a video generation task with Seedance 2.0 (alias: gen)",
                "aliases": ["gen"],
                "args": [],
                "options": [
                    {"name": "--prompt",            "short": "-p", "type": "string", "required": false, "description": "Text prompt (supports [Image N], [Video N], [Audio N], time codes like [0-4s])"},
                    {"name": "--image",             "short": "-i", "type": "string", "required": false, "multiple": true, "description": "Reference image path or URL (repeatable, max 9; role=reference_image)"},
                    {"name": "--first-frame",                      "type": "string", "required": false, "description": "Image used as first frame (role=first_frame)"},
                    {"name": "--last-frame",                       "type": "string", "required": false, "description": "Image used as last frame (role=last_frame, requires --first-frame)"},
                    {"name": "--video",             "short": "-v", "type": "string", "required": false, "multiple": true, "description": "Reference video URL (repeatable, max 3, local paths not supported)"},
                    {"name": "--audio",             "short": "-a", "type": "string", "required": false, "multiple": true, "description": "Reference audio path or URL (wav/mp3, repeatable, max 3; requires at least one image or video)"},
                    {"name": "--duration",          "short": "-d", "type": "integer", "required": false, "default": 5, "description": "Seconds [4,15] or -1 for auto"},
                    {"name": "--resolution",        "short": "-r", "type": "string", "required": false, "default": "720p", "values": ["480p", "720p"], "description": "Output resolution (Seedance 2.0 has no 1080p)"},
                    {"name": "--ratio",                            "type": "string", "required": false, "default": "adaptive", "values": ["16:9","4:3","1:1","3:4","9:16","21:9","adaptive"], "description": "Aspect ratio"},
                    {"name": "--seed",                             "type": "integer", "required": false, "default": -1, "description": "Seed; -1 = random"},
                    {"name": "--audio-sync",                       "type": "bool", "required": false, "default": true, "description": "Generate synchronized audio"},
                    {"name": "--no-audio-sync",                    "type": "bool", "required": false, "default": false, "description": "Output silent video"},
                    {"name": "--watermark",                        "type": "bool", "required": false, "default": false, "description": "Add ModelArk watermark"},
                    {"name": "--fast",                             "type": "bool", "required": false, "default": false, "description": "Use Seedance 2.0 Fast"},
                    {"name": "--model",                            "type": "string", "required": false, "description": "Override model id"},
                    {"name": "--callback-url",                     "type": "string", "required": false, "description": "Webhook to notify on status change"},
                    {"name": "--safety-identifier",                "type": "string", "required": false, "description": "Hashed end-user id (<=64 ASCII chars)"},
                    {"name": "--wait",              "short": "-w", "type": "bool", "required": false, "description": "Block until the task finishes"},
                    {"name": "--output",            "short": "-o", "type": "path", "required": false, "description": "Output file path (implies --wait)"},
                    {"name": "--poll-interval",                    "type": "integer", "required": false, "default": 5, "description": "Seconds between polls while waiting"},
                    {"name": "--timeout",                          "type": "integer", "required": false, "default": 900, "description": "Max wait seconds (0 = unlimited)"},
                    {"name": "--api-key",                          "type": "string", "required": false, "description": "API key override (else SEEDANCE_API_KEY / ARK_API_KEY / config)"}
                ]
            },
            "status": {
                "description": "Retrieve a video generation task (alias: get)",
                "aliases": ["get"],
                "args": [{"name": "id", "kind": "positional", "type": "string", "required": true}],
                "options": [
                    {"name": "--api-key", "type": "string", "required": false}
                ]
            },
            "download": {
                "description": "Download the video for a completed task",
                "args": [{"name": "id", "kind": "positional", "type": "string", "required": true}],
                "options": [
                    {"name": "--output", "short": "-o", "type": "path", "required": false, "description": "Output file path (default: <id>.mp4)"},
                    {"name": "--api-key", "type": "string", "required": false}
                ]
            },
            "cancel": {
                "description": "Cancel a queued task (alias: rm)",
                "aliases": ["rm"],
                "args": [{"name": "id", "kind": "positional", "type": "string", "required": true}],
                "options": [
                    {"name": "--api-key", "type": "string", "required": false}
                ]
            },
            "character-sheet": {
                "description": "Build a 9-angle character reference sheet from a single photo via nanaban (Nano Banana Pro). Resulting PNG can be passed to `generate --image` to keep a specific person consistent across Seedance shots -- works around the single-face upload block.",
                "args": [
                    {"name": "input", "kind": "positional", "type": "string", "required": true, "description": "Path or URL of the subject photo"}
                ],
                "options": [
                    {"name": "--output", "short": "-o", "type": "path",   "required": false, "description": "Output PNG path (default: ~/Documents/seedance/character-sheet-<hash>.png)"},
                    {"name": "--style",                   "type": "string", "required": false, "description": "Extra styling hints to append to the grid prompt"},
                    {"name": "--angles",                  "type": "integer", "required": false, "default": 9, "values": [4, 9], "description": "9 (3x3) or 4 (2x2) cells"}
                ],
                "requires": ["nanaban"],
                "credit": "Community trick originated by @wtry1102 / @voxelplot Advanced Workflow #8"
            },
            "audio-to-video": {
                "description": "Wrap an audio file inside a silent mp4 so it can be fed as `--video`. Workaround for Seedance 2.0's reference-audio-mutates-lyrics quirk. Uses ffmpeg.",
                "args": [
                    {"name": "input", "kind": "positional", "type": "path", "required": true, "description": "Input audio file (wav/mp3/m4a/etc)"}
                ],
                "options": [
                    {"name": "--output",     "short": "-o", "type": "path",    "required": false, "description": "Output mp4 path (default: <input>.silent.mp4)"},
                    {"name": "--background",                "type": "string",  "required": false, "default": "black", "values": ["black","white"]},
                    {"name": "--height",                    "type": "integer", "required": false, "default": 480, "values": [480, 720]}
                ],
                "requires": ["ffmpeg"],
                "credit": "@simeonnz via @MrDavids1"
            },
            "models": {
                "description": "List available Seedance model ids (alias: ls)",
                "aliases": ["ls"],
                "args": [],
                "options": []
            },
            "doctor": {
                "description": "Check API key, base URL, and dependency health",
                "args": [],
                "options": []
            },
            "agent-info": {
                "description": "This manifest",
                "aliases": ["info"],
                "args": [],
                "options": []
            },
            "skill install": {"description": "Install skill file to agent platforms", "args": [], "options": []},
            "skill status":  {"description": "Check skill installation status",        "args": [], "options": []},
            "config show":   {"description": "Display effective merged configuration (api_key masked)", "args": [], "options": []},
            "config path":   {"description": "Show configuration file path",            "args": [], "options": []},
            "config set":    {
                "description": "Persist a setting in the config file (chmod 600)",
                "args": [
                    {"name": "key",   "kind": "positional", "type": "string", "required": true, "values": ["api-key","base-url","model"]},
                    {"name": "value", "kind": "positional", "type": "string", "required": true}
                ],
                "options": []
            },
            "config unset":  {
                "description": "Remove a setting from the config file",
                "args": [
                    {"name": "key", "kind": "positional", "type": "string", "required": true, "values": ["api-key","base-url","model"]}
                ],
                "options": []
            },
            "update": {
                "description": "Self-update binary from GitHub Releases",
                "args": [],
                "options": [{"name": "--check", "type": "bool", "required": false, "default": false}]
            }
        },
        "global_flags": {
            "--json":  {"description": "Force JSON output (auto-enabled when piped)", "type": "bool", "default": false},
            "--quiet": {"description": "Suppress informational output",               "type": "bool", "default": false}
        },
        "exit_codes": {
            "0": "Success",
            "1": "Transient error (IO, network, API) -- retry",
            "2": "Config error -- fix setup",
            "3": "Bad input -- fix arguments",
            "4": "Rate limited -- wait and retry"
        },
        "envelope": {
            "version": "1",
            "success": "{ version, status, data }",
            "error": "{ version, status, error: { code, message, suggestion } }"
        },
        "config": {
            "path": config_path.display().to_string(),
            "env_prefix": "SEEDANCE_",
            "fallback_env_keys": ["SEEDANCE_API_KEY", "ARK_API_KEY"]
        },
        "api": {
            "provider": "BytePlus ModelArk",
            "base_url_default": crate::config::DEFAULT_BASE_URL,
            "default_model": crate::config::DEFAULT_MODEL,
            "fast_model": crate::config::DEFAULT_MODEL_FAST,
            "reference_limits": {
                "images": "0-9",
                "videos": "0-3 URLs, total <=15s, local paths not supported by API",
                "audio": "0-3 (wav/mp3), total <=15s, must accompany at least one image or video"
            },
            "prompt_syntax": "Use [Image 1]..[Image N], [Video 1]..[Video 3], [Audio 1]..[Audio 3], and time codes like `[0-4s]: ...`"
        },
        "auto_json_when_piped": true,
        "companion_tools": {
            "nanaban": {
                "purpose": "Image generation. Used by `seedance character-sheet` to render a 9-angle reference grid of a specific person (Nano Banana Pro / Gemini 3 Pro image).",
                "install": "npm i -g nanaban",
                "repo": "https://github.com/199-biotechnologies/nanaban",
                "required_for": ["character-sheet"]
            },
            "ffmpeg": {
                "purpose": "Media transcoding. Used by `seedance audio-to-video` to wrap audio in a silent mp4 (workaround for the lyrics-mutation quirk).",
                "install": "brew install ffmpeg (macOS) or apt install ffmpeg (linux)",
                "required_for": ["audio-to-video"]
            }
        },
        "agent_workflows": {
            "consistent_person_across_shots": [
                "1. seedance character-sheet <single_photo> -o sheet.png",
                "2. seedance generate --image sheet.png --prompt '...(subject from [Image 1]) ...' --wait",
                "(The grid acts as a multi-angle subject reference. Do NOT also use --first-frame with the same photo -- multi-mode conflict.)"
            ],
            "exact_music_or_dialogue_preserved": [
                "1. seedance audio-to-video song.mp3 -o song.silent.mp4",
                "2. Host the mp4 at a public URL (S3 signed URL / catbox.moe / your CDN)",
                "3. seedance generate --video <hosted_url> --prompt '...use [Video 1] as the soundtrack throughout...' --image <subject> --wait",
                "(Passing raw --audio instead would let Seedance rewrite lyrics and melody.)"
            ]
        }
    });
    println!("{}", serde_json::to_string_pretty(&info).unwrap());
}
