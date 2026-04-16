use serde::Serialize;

use crate::cli::ConfigKey;
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

// ── config set / unset ────────────────────────────────────────────────────

#[derive(Serialize)]
struct MutateResult {
    path: String,
    key: String,
    value_display: String,
    action: &'static str,
}

pub fn set(ctx: Ctx, key: ConfigKey, value: String) -> Result<(), AppError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "value for {} cannot be empty",
            key.as_str()
        )));
    }

    let cfg_path = config::config_path();
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut doc = read_doc(&cfg_path)?;
    doc[key.as_str()] = toml_edit::value(value.as_str());
    write_doc(&cfg_path, &doc)?;

    // Permissions: config contains the API key -- lock it down on Unix.
    restrict_permissions(&cfg_path)?;

    let value_display = match key {
        ConfigKey::ApiKey => config::mask_secret(&value),
        _ => value,
    };
    let result = MutateResult {
        path: cfg_path.display().to_string(),
        key: key.as_str().into(),
        value_display,
        action: "set",
    };
    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!(
            "{} {} = {} {}",
            "set".green(),
            r.key.bold(),
            r.value_display.cyan(),
            format_args!("({})", r.path).to_string().dimmed()
        );
    });
    Ok(())
}

pub fn unset(ctx: Ctx, key: ConfigKey) -> Result<(), AppError> {
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        return Err(AppError::Config(format!(
            "no config file at {}",
            cfg_path.display()
        )));
    }
    let mut doc = read_doc(&cfg_path)?;
    let removed = doc.remove(key.as_str()).is_some();
    write_doc(&cfg_path, &doc)?;

    let result = MutateResult {
        path: cfg_path.display().to_string(),
        key: key.as_str().into(),
        value_display: if removed { "removed" } else { "already-absent" }.into(),
        action: "unset",
    };
    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!(
            "{} {} ({})",
            "unset".yellow(),
            r.key.bold(),
            r.path.dimmed()
        );
    });
    Ok(())
}

fn read_doc(path: &std::path::Path) -> Result<toml_edit::DocumentMut, AppError> {
    if !path.exists() {
        return Ok(toml_edit::DocumentMut::new());
    }
    let body = std::fs::read_to_string(path)?;
    body.parse::<toml_edit::DocumentMut>()
        .map_err(|e| AppError::Config(format!("config is not valid TOML: {e}")))
}

fn write_doc(path: &std::path::Path, doc: &toml_edit::DocumentMut) -> Result<(), AppError> {
    std::fs::write(path, doc.to_string())?;
    Ok(())
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) -> Result<(), AppError> {
    Ok(())
}
