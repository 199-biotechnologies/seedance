use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "seedance",
    version,
    about = "Generate video with ByteDance Seedance 2.0 from the terminal.",
    long_about = "Generate video with ByteDance Seedance 2.0 via the BytePlus ModelArk API.

Supports text-to-video, image-to-video (first / first+last / up to 9 reference images),
reference videos, reference audio, and multimodal mixes. Use time-coded prompts like
`[Image 1] ... [Video 1] ... [Audio 1] ...` and `[0-4s]: shot description` for multi-shot control.",
    after_long_help = HELP_FOOTER,
)]
pub struct Cli {
    /// Force JSON output even in a terminal
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress informational output
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

const HELP_FOOTER: &str = "\
Tips:
  * Run `seedance agent-info | jq` for the full capability manifest
  * Get an API key from https://console.byteplus.com/ark, then save it with:
      seedance config set api-key ark-xxxxxxxx (stored at chmod 600, never echoed)
    or export SEEDANCE_API_KEY / ARK_API_KEY
  * Reference files: images and audio can be local paths (base64-encoded inline) OR URLs
  * Videos must be URLs -- the API does not accept base64 for video
  * Audio alone is not allowed -- Seedance requires at least one image or video alongside audio
  * Known quirk: uploading audio mutates lyrics. Workaround: render a silent MP4 with the audio
    baked in, then pass it as --video (credit: @simeonnz via @MrDavids1)
  * Use --wait to block until the task finishes and download the result in one command
  * Real human faces in references are blocked -- use faces from previously generated Seedance videos
  * Default output dir: ~/Documents/seedance/<task-id>.mp4 (override with -o /path/to/file.mp4)

Examples:
  seedance generate --prompt \"A cat yawns at the camera\" --wait --output cat.mp4
    Text-to-video, blocks until the mp4 lands on disk

  seedance generate --prompt \"[Image 1] the boy waves\" --image boy.png --duration 8 --wait -o out.mp4
    Single reference image + prompt, 8 seconds, wait and download

  seedance generate --first-frame first.png --last-frame last.png --prompt \"morph between them\"
    First+last frame mode -- returns a task id, poll separately

  seedance generate --prompt \"...\" --image a.png --image b.png --video ref.mp4 --fast --wait -o out.mp4
    Multimodal reference-to-video using the fast tier

  seedance status cgt-20260416-abcd1234
    Poll a task; prints video_url when succeeded

  seedance download cgt-20260416-abcd1234 --output final.mp4
    Download the video for a completed task

  seedance doctor
    Verify API key + base URL reachability before running a real generation";

#[derive(Clone, Copy, ValueEnum, serde::Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    #[value(name = "480p")]
    P480,
    #[value(name = "720p")]
    P720,
}

impl Resolution {
    pub fn as_api(&self) -> &'static str {
        match self {
            Self::P480 => "480p",
            Self::P720 => "720p",
        }
    }
}

#[derive(Clone, Copy, ValueEnum, serde::Serialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Ratio {
    #[value(name = "16:9")]
    Sixteen9,
    #[value(name = "4:3")]
    Four3,
    #[value(name = "1:1")]
    One1,
    #[value(name = "3:4")]
    Three4,
    #[value(name = "9:16")]
    Nine16,
    #[value(name = "21:9")]
    TwentyOne9,
    Adaptive,
}

impl Ratio {
    pub fn as_api(&self) -> &'static str {
        match self {
            Self::Sixteen9 => "16:9",
            Self::Four3 => "4:3",
            Self::One1 => "1:1",
            Self::Three4 => "3:4",
            Self::Nine16 => "9:16",
            Self::TwentyOne9 => "21:9",
            Self::Adaptive => "adaptive",
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a video generation task
    #[command(visible_alias = "gen")]
    Generate(Box<GenerateArgs>),

    /// Retrieve a video generation task by id
    #[command(visible_alias = "get")]
    Status {
        /// Task id (e.g. cgt-20260416-abcd1234)
        id: String,
        /// API key override (else SEEDANCE_API_KEY / ARK_API_KEY / config)
        #[arg(long, env = "SEEDANCE_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
    },

    /// Download the generated video for a completed task
    Download {
        /// Task id
        id: String,
        /// Output file path (default: <id>.mp4 in current dir)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
        /// API key override
        #[arg(long, env = "SEEDANCE_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
    },

    /// Cancel a queued task (only possible while status=queued)
    #[command(visible_alias = "rm")]
    Cancel {
        /// Task id
        id: String,
        /// API key override
        #[arg(long, env = "SEEDANCE_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
    },

    /// List available Seedance model ids
    #[command(visible_alias = "ls")]
    Models,

    /// Check API key, base URL, and dependency health
    Doctor,

    /// Machine-readable capability manifest
    #[command(visible_alias = "info")]
    AgentInfo,

    /// Manage skill file installation for AI agent platforms
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Self-update from GitHub Releases
    Update {
        /// Check only, don't install
        #[arg(long)]
        check: bool,
    },
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// Text prompt. Use [Image N] / [Video N] / [Audio N] to reference inputs,
    /// and time codes like `[0-4s]: shot description` for multi-shot control.
    #[arg(long, short = 'p')]
    pub prompt: Option<String>,

    /// Reference image (local path or URL). Repeatable, up to 9.
    /// Role is `reference_image`. Use --first-frame / --last-frame for those modes.
    #[arg(long = "image", short = 'i', value_name = "PATH|URL")]
    pub images: Vec<String>,

    /// Image used as the first frame (role=first_frame)
    #[arg(long, value_name = "PATH|URL", conflicts_with = "images")]
    pub first_frame: Option<String>,

    /// Image used as the last frame (role=last_frame). Requires --first-frame.
    #[arg(long, value_name = "PATH|URL", requires = "first_frame", conflicts_with = "images")]
    pub last_frame: Option<String>,

    /// Reference video URL (role=reference_video). Repeatable, up to 3, total <=15s.
    /// Local paths are NOT supported by the API -- upload to a URL first.
    #[arg(long = "video", short = 'v', value_name = "URL")]
    pub videos: Vec<String>,

    /// Reference audio (local path or URL, wav/mp3). Repeatable, up to 3, total <=15s.
    /// Cannot be the only reference -- requires at least one image or video.
    #[arg(long = "audio", short = 'a', value_name = "PATH|URL")]
    pub audio: Vec<String>,

    /// Video duration in seconds. [4,15] or -1 for auto (Seedance 2.0).
    #[arg(long, short = 'd', default_value_t = 5, allow_hyphen_values = true)]
    pub duration: i32,

    /// Output resolution. Seedance 2.0 does not support 1080p.
    #[arg(long, short = 'r', value_enum, default_value = "720p")]
    pub resolution: Resolution,

    /// Output aspect ratio
    #[arg(long, value_enum, default_value = "adaptive")]
    pub ratio: Ratio,

    /// Seed for reproducibility. -1 = random.
    #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
    pub seed: i64,

    /// Generate audio synchronized with the video (default)
    #[arg(long = "audio-sync", default_value_t = true, overrides_with = "no_audio_sync")]
    pub audio_sync: bool,

    /// Output a silent video
    #[arg(long = "no-audio-sync", default_value_t = false)]
    pub no_audio_sync: bool,

    /// Add a ModelArk watermark to the output
    #[arg(long)]
    pub watermark: bool,

    /// Use the Seedance 2.0 Fast tier (lower latency + cost, slight quality tradeoff)
    #[arg(long, conflicts_with = "model")]
    pub fast: bool,

    /// Override model id (default: dreamina-seedance-2-0-260128, or the fast variant with --fast)
    #[arg(long)]
    pub model: Option<String>,

    /// Callback URL the API hits on status change (optional)
    #[arg(long, value_name = "URL")]
    pub callback_url: Option<String>,

    /// Hashed end-user id for abuse tracking (optional, <=64 ASCII chars)
    #[arg(long, value_name = "ID")]
    pub safety_identifier: Option<String>,

    /// Block until the task finishes, then optionally download the video
    #[arg(long, short = 'w')]
    pub wait: bool,

    /// Output file path (implies --wait). Defaults to <id>.mp4 when --wait is set alone.
    #[arg(long, short = 'o', value_name = "PATH")]
    pub output: Option<std::path::PathBuf>,

    /// Poll interval in seconds when --wait is set
    #[arg(long, default_value_t = 5)]
    pub poll_interval: u64,

    /// Maximum wait in seconds when --wait is set (0 = no limit)
    #[arg(long, default_value_t = 900)]
    pub timeout: u64,

    /// API key override (else SEEDANCE_API_KEY / ARK_API_KEY / config)
    #[arg(long, env = "SEEDANCE_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// Write skill file to all detected agent platforms
    Install,
    /// Check which platforms have the skill installed
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Display effective merged configuration
    Show,
    /// Print configuration file path
    Path,
    /// Write a value into the TOML config file (api-key, base-url, model)
    Set {
        /// Which setting to update
        #[arg(value_enum)]
        key: ConfigKey,
        /// New value
        value: String,
    },
    /// Remove a value from the TOML config file
    Unset {
        #[arg(value_enum)]
        key: ConfigKey,
    },
}

#[derive(Clone, Copy, ValueEnum, Debug)]
#[value(rename_all = "kebab-case")]
pub enum ConfigKey {
    /// BytePlus ModelArk API key (stored locally, never echoed in `config show`)
    ApiKey,
    /// API base URL (override if BytePlus publishes a new region)
    BaseUrl,
    /// Default model id
    Model,
}

impl ConfigKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::BaseUrl => "base_url",
            Self::Model => "model",
        }
    }
}
