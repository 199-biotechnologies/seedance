/// Configuration loading with 3-tier precedence:
///   1. Compiled defaults
///   2. TOML config file (~/.config/seedance/config.toml)
///   3. Environment variables (SEEDANCE_*)
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::AppError;

pub const DEFAULT_BASE_URL: &str = "https://ark.ap-southeast.bytepluses.com/api/v3";
pub const DEFAULT_MODEL: &str = "dreamina-seedance-2-0-260128";
pub const DEFAULT_MODEL_FAST: &str = "dreamina-seedance-2-0-fast-260128";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// ModelArk base URL. Override only if BytePlus publishes a new region.
    pub base_url: String,

    /// Default model id
    pub model: String,

    /// API key (prefer env var SEEDANCE_API_KEY or ARK_API_KEY over storing here)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Self-update settings
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub enabled: bool,
    pub owner: String,
    pub repo: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            model: DEFAULT_MODEL.into(),
            api_key: None,
            update: UpdateConfig::default(),
        }
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            owner: "199-biotechnologies".into(),
            repo: "seedance".into(),
        }
    }
}

pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("config.toml")
}

pub fn load() -> Result<AppConfig, AppError> {
    use figment::Figment;
    use figment::providers::{Env, Format as _, Serialized, Toml};

    Figment::from(Serialized::defaults(AppConfig::default()))
        .merge(Toml::file(config_path()))
        .merge(Env::prefixed("SEEDANCE_").split("__"))
        .extract()
        .map_err(|e| AppError::Config(e.to_string()))
}

/// Resolve the API key from flag -> env vars (SEEDANCE_API_KEY, ARK_API_KEY) -> config.
/// Returns None if no source provides a non-empty key.
pub fn resolve_api_key(flag: Option<&str>, cfg: &AppConfig) -> Option<String> {
    if let Some(v) = flag {
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    for env in ["SEEDANCE_API_KEY", "ARK_API_KEY"] {
        if let Ok(v) = std::env::var(env) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    cfg.api_key
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "(not set)".to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        let prefix: String = chars[..2.min(chars.len())].iter().collect();
        format!("{prefix}***")
    } else {
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{prefix}...{suffix}")
    }
}
