use serde::Serialize;

use crate::api::ApiClient;
use crate::config::{self, AppConfig};
use crate::error::AppError;
use crate::output::{self, Ctx};

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: CheckStatus,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

#[derive(Serialize)]
struct DoctorSummary {
    pass: usize,
    warn: usize,
    fail: usize,
}

#[derive(Serialize)]
struct DoctorReport {
    checks: Vec<DoctorCheck>,
    summary: DoctorSummary,
}

fn companion_check(binary: &'static str, install_hint: &'static str, unlocks: &'static str) -> DoctorCheck {
    match which::which(binary) {
        Ok(path) => DoctorCheck {
            name: binary,
            status: CheckStatus::Pass,
            message: format!("{} ({})", path.display(), unlocks),
            suggestion: None,
        },
        Err(_) => DoctorCheck {
            name: binary,
            status: CheckStatus::Warn,
            message: format!("{binary} not on PATH -- {unlocks}"),
            suggestion: Some(format!("Optional. Install: {install_hint}")),
        },
    }
}

fn is_not_found(code: &str) -> bool {
    let lower = code.to_ascii_lowercase();
    code == "404"
        || lower.contains("notfound")
        || lower.contains("not_found")
        || lower.contains("resource_not_found")
        || lower.contains("resourcenotfound")
}

fn is_auth_failure(code: &str) -> bool {
    let lower = code.to_ascii_lowercase();
    matches!(code, "401" | "403")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("authfail")
        || lower.contains("auth_fail")
        || lower.contains("invalidapikey")
        || lower.contains("invalid_api_key")
}

pub fn run(ctx: Ctx, cfg: &AppConfig) -> Result<(), AppError> {
    let mut checks: Vec<DoctorCheck> = Vec::new();

    // Config file presence
    let cfg_path = config::config_path();
    checks.push(if cfg_path.exists() {
        DoctorCheck {
            name: "config_file",
            status: CheckStatus::Pass,
            message: cfg_path.display().to_string(),
            suggestion: None,
        }
    } else {
        DoctorCheck {
            name: "config_file",
            status: CheckStatus::Warn,
            message: format!("{} not found (using defaults)", cfg_path.display()),
            suggestion: Some(format!(
                "Create it with: seedance config show > {}",
                cfg_path.display()
            )),
        }
    });

    // API key resolution
    let resolved = config::resolve_api_key(None, cfg);
    checks.push(match &resolved {
        Some(k) => DoctorCheck {
            name: "api_key",
            status: CheckStatus::Pass,
            message: format!("found ({})", config::mask_secret(k)),
            suggestion: None,
        },
        None => DoctorCheck {
            name: "api_key",
            status: CheckStatus::Fail,
            message: "no API key found".into(),
            suggestion: Some(
                "Get one at https://console.byteplus.com/ark and export SEEDANCE_API_KEY=..."
                    .into(),
            ),
        },
    });

    // Base URL reachability (only if we have a key, to avoid a pointless 401)
    if let Some(key) = resolved.as_deref() {
        let ping = ApiClient::new(&cfg.base_url, key).and_then(|c| {
            // Call GET on a clearly non-existent task id: any "not found" response
            // proves the API is reachable and auth is valid.
            c.get_task("cgt-seedance-cli-doctor-ping")
        });
        checks.push(match ping {
            Ok(_) => DoctorCheck {
                name: "base_url",
                status: CheckStatus::Pass,
                message: format!("{} reachable", cfg.base_url),
                suggestion: None,
            },
            Err(AppError::Api { code, .. }) if is_not_found(&code) => DoctorCheck {
                name: "base_url",
                status: CheckStatus::Pass,
                message: format!("{} reachable (auth OK)", cfg.base_url),
                suggestion: None,
            },
            Err(AppError::Api { code, message }) if is_auth_failure(&code) => DoctorCheck {
                name: "base_url",
                status: CheckStatus::Fail,
                message: format!("auth rejected: {code} {message}"),
                suggestion: Some(
                    "Check that SEEDANCE_API_KEY matches an active key in the BytePlus console"
                        .into(),
                ),
            },
            Err(e) => DoctorCheck {
                name: "base_url",
                status: CheckStatus::Fail,
                message: format!("unreachable: {e}"),
                suggestion: Some("Verify network connectivity and base_url".into()),
            },
        });
    } else {
        checks.push(DoctorCheck {
            name: "base_url",
            status: CheckStatus::Warn,
            message: format!("{} (skipped -- no API key to test auth)", cfg.base_url),
            suggestion: None,
        });
    }

    // Companion tools -- not hard requirements, but unlock feature subcommands.
    checks.push(companion_check(
        "nanaban",
        "npm i -g nanaban (or see https://github.com/paperfoot/nanaban-cli)",
        "unlocks `seedance character-sheet` for consistent-person generations",
    ));
    checks.push(companion_check(
        "ffmpeg",
        "brew install ffmpeg (macOS) or apt install ffmpeg (linux)",
        "unlocks `seedance audio-to-video` (the @simeonnz lyrics-preservation trick)",
    ));

    let summary = DoctorSummary {
        pass: checks.iter().filter(|c| c.status == CheckStatus::Pass).count(),
        warn: checks.iter().filter(|c| c.status == CheckStatus::Warn).count(),
        fail: checks.iter().filter(|c| c.status == CheckStatus::Fail).count(),
    };
    let has_failures = summary.fail > 0;

    let report = DoctorReport { checks, summary };
    output::print_success_or(ctx, &report, |r| {
        use owo_colors::OwoColorize;
        for check in &r.checks {
            let (icon, colored) = match check.status {
                CheckStatus::Pass => ("[ok]", "[ok]".green().to_string()),
                CheckStatus::Warn => ("[warn]", "[warn]".yellow().to_string()),
                CheckStatus::Fail => ("[fail]", "[fail]".red().to_string()),
            };
            let _ = icon;
            println!(
                "{} {}: {}",
                colored,
                check.name.bold(),
                check.message
            );
            if let Some(s) = &check.suggestion {
                println!("    {}", s.dimmed());
            }
        }
    });

    if has_failures {
        return Err(AppError::Config(
            "doctor found issues. Run with --json for structured details.".into(),
        ));
    }
    Ok(())
}
