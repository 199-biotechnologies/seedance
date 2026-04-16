use crate::api::ApiClient;
use crate::config;
use crate::error::AppError;
use crate::output::{self, Ctx};

pub fn run(ctx: Ctx, id: String, api_key: Option<String>) -> Result<(), AppError> {
    let cfg = config::load()?;
    let key = config::resolve_api_key(api_key.as_deref(), &cfg).ok_or_else(|| {
        AppError::Config(
            "no API key found. Set SEEDANCE_API_KEY (or ARK_API_KEY) or pass --api-key.".into(),
        )
    })?;
    let api = ApiClient::new(&cfg.base_url, &key)?;
    let task = api.get_task(&id)?;

    output::print_success_or(ctx, &task, |t| {
        use owo_colors::OwoColorize;
        let status_styled = match t.status.as_str() {
            "succeeded" => t.status.green().to_string(),
            "failed" | "cancelled" | "expired" => t.status.red().to_string(),
            _ => t.status.yellow().to_string(),
        };
        println!("{} {}", "id:".bold(), t.id.cyan());
        println!("{} {}", "status:".bold(), status_styled);
        if let Some(m) = &t.model {
            println!("model: {m}");
        }
        if let Some(c) = &t.content
            && let Some(url) = &c.video_url
        {
            println!("video: {}", url.cyan());
        }
        if let Some(err) = &t.error {
            println!(
                "error: {} {}",
                err.code.clone().unwrap_or_default().red(),
                err.message.clone().unwrap_or_default()
            );
        }
    });
    Ok(())
}
