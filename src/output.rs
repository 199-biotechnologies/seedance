/// Output format detection and JSON envelope helpers.
use serde::Serialize;
use std::io::IsTerminal;

use crate::error::AppError;

#[derive(Clone, Copy)]
pub enum Format {
    Json,
    Human,
}

impl Format {
    pub fn detect(json_flag: bool) -> Self {
        if json_flag || !std::io::stdout().is_terminal() {
            Format::Json
        } else {
            Format::Human
        }
    }

    #[allow(dead_code)]
    pub fn is_json(self) -> bool {
        matches!(self, Format::Json)
    }
}

#[derive(Clone, Copy)]
pub struct Ctx {
    pub format: Format,
    pub quiet: bool,
}

impl Ctx {
    pub fn new(json_flag: bool, quiet: bool) -> Self {
        Self {
            format: Format::detect(json_flag),
            quiet,
        }
    }
}

fn safe_json_string<T: Serialize>(value: &T) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(s) => s,
        Err(e) => {
            let fallback = serde_json::json!({
                "version": "1",
                "status": "error",
                "error": {
                    "code": "serialize",
                    "message": e.to_string(),
                    "suggestion": "Retry the command",
                },
            });
            serde_json::to_string_pretty(&fallback).unwrap_or_else(|_| {
                r#"{"version":"1","status":"error","error":{"code":"serialize","message":"serialization failed","suggestion":"Retry the command"}}"#.to_string()
            })
        }
    }
}

pub fn print_success_or<T: Serialize, F: FnOnce(&T)>(ctx: Ctx, data: &T, human: F) {
    match ctx.format {
        Format::Json => {
            let envelope = serde_json::json!({
                "version": "1",
                "status": "success",
                "data": data,
            });
            println!("{}", safe_json_string(&envelope));
        }
        Format::Human if !ctx.quiet => human(data),
        Format::Human => {}
    }
}

pub fn print_error(format: Format, err: &AppError) {
    let envelope = serde_json::json!({
        "version": "1",
        "status": "error",
        "error": {
            "code": err.error_code(),
            "message": err.to_string(),
            "suggestion": err.suggestion(),
        },
    });
    match format {
        Format::Json => eprintln!("{}", safe_json_string(&envelope)),
        Format::Human => {
            use owo_colors::OwoColorize;
            eprintln!("{} {}", "error:".red().bold(), err);
            eprintln!("  {}", err.suggestion().dimmed());
        }
    }
}

pub fn print_help_json(err: clap::Error) {
    let envelope = serde_json::json!({
        "version": "1",
        "status": "success",
        "data": { "usage": err.to_string().trim_end() },
    });
    println!("{}", safe_json_string(&envelope));
}

pub fn print_clap_error(format: Format, err: &clap::Error) {
    match format {
        Format::Json => {
            let envelope = serde_json::json!({
                "version": "1",
                "status": "error",
                "error": {
                    "code": "invalid_input",
                    "message": err.to_string(),
                    "suggestion": "Check arguments with: seedance --help",
                },
            });
            eprintln!("{}", safe_json_string(&envelope));
        }
        Format::Human => {
            eprint!("{err}");
        }
    }
}

/// Status messages for human mode during long-running operations.
/// Always goes to stderr so stdout stays clean for JSON / pipe output.
#[allow(dead_code)]
pub fn info(ctx: Ctx, msg: &str) {
    if ctx.quiet {
        return;
    }
    if matches!(ctx.format, Format::Human) {
        use owo_colors::OwoColorize;
        eprintln!("{} {}", "::".cyan(), msg);
    }
}
