use serde::Serialize;

use crate::config::{self, AppConfig};
use crate::error::AppError;
use crate::output::{self, Ctx};

pub fn show(ctx: Ctx, cfg: &AppConfig) -> Result<(), AppError> {
    // Mask the key for display; never emit plaintext.
    let masked = MaskedConfig {
        base_url: &cfg.base_url,
        model: &cfg.model,
        api_key: cfg.api_key.as_deref().map(config::mask_secret),
        update: &cfg.update,
    };
    output::print_success_or(ctx, &masked, |c| {
        println!("{}", serde_json::to_string_pretty(c).unwrap());
    });
    Ok(())
}

#[derive(Serialize)]
struct MaskedConfig<'a> {
    base_url: &'a str,
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    update: &'a crate::config::UpdateConfig,
}

#[derive(Serialize)]
struct ConfigPath {
    path: String,
    exists: bool,
}

pub fn path(ctx: Ctx) -> Result<(), AppError> {
    let p = config::config_path();
    let result = ConfigPath {
        path: p.display().to_string(),
        exists: p.exists(),
    };
    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{}", r.path);
        if !r.exists {
            println!("  {}", "(file does not exist, using defaults)".dimmed());
        }
    });
    Ok(())
}
