use serde::Serialize;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::output::{self, Ctx};

#[derive(Serialize)]
struct UpdateResult {
    current_version: String,
    latest_version: String,
    status: String,
}

pub fn run(ctx: Ctx, check: bool, config: &AppConfig) -> Result<(), AppError> {
    let current = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    if !config.update.enabled {
        let result = UpdateResult {
            current_version: current.into(),
            latest_version: current.into(),
            status: "disabled".into(),
        };
        output::print_success_or(ctx, &result, |_| {
            println!("Self-update is disabled in config");
        });
        return Ok(());
    }

    let updater = self_update::backends::github::Update::configure()
        .repo_owner(&config.update.owner)
        .repo_name(&config.update.repo)
        .bin_name(name)
        .current_version(current)
        .build()
        .map_err(|e| AppError::Update(e.to_string()))?;

    if check {
        let latest = updater
            .get_latest_release()
            .map_err(|e| AppError::Update(e.to_string()))?;
        let v = latest.version.trim_start_matches('v').to_string();
        let up_to_date = v == current;
        let result = UpdateResult {
            current_version: current.into(),
            latest_version: v,
            status: if up_to_date {
                "up_to_date".into()
            } else {
                "update_available".into()
            },
        };
        output::print_success_or(ctx, &result, |r| {
            if up_to_date {
                println!("Up to date (v{})", r.current_version);
            } else {
                println!(
                    "Update available: v{} -> v{}",
                    r.current_version, r.latest_version
                );
                println!("Run `{name} update` to install");
            }
        });
    } else {
        let release = updater
            .update()
            .map_err(|e| AppError::Update(e.to_string()))?;
        let v = release.version().trim_start_matches('v').to_string();
        let up_to_date = v == current;
        let result = UpdateResult {
            current_version: current.into(),
            latest_version: v,
            status: if up_to_date {
                "up_to_date".into()
            } else {
                "updated".into()
            },
        };
        output::print_success_or(ctx, &result, |r| {
            if up_to_date {
                println!("Already up to date (v{})", r.current_version);
            } else {
                println!("Updated: v{} -> v{}", r.current_version, r.latest_version);
                println!("Run `{name} skill install` to update agent skills");
            }
        });
    }
    Ok(())
}
