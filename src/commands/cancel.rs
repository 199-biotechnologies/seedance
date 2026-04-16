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
    let result = api.cancel_task(&id)?;

    output::print_success_or(ctx, &result, |r| {
        use owo_colors::OwoColorize;
        println!("{} {}", "cancelled:".bold(), r.to_string().cyan());
    });
    Ok(())
}
