//! seedance -- ByteDance Seedance 2.0 video generation CLI.
//!
//! Patterns from agent-cli-framework:
//!   - JSON envelope on stdout, coloured table on TTY
//!   - Semantic exit codes (0-4)
//!   - `agent-info` capability discovery
//!   - `skill install`, `config show/path`, `doctor`, `update`
//!   - Pre-scan --json, parse with try_parse, own the exit code.

mod api;
mod cli;
mod commands;
mod config;
mod error;
mod media;
mod output;

use clap::Parser;

use cli::{Cli, Commands, ConfigAction, SkillAction};
use output::{Ctx, Format};

fn has_json_flag() -> bool {
    std::env::args_os().any(|a| a == "--json")
}

fn main() {
    let json_flag = has_json_flag();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
            ) {
                let format = Format::detect(json_flag);
                match format {
                    Format::Json => {
                        output::print_help_json(e);
                        std::process::exit(0);
                    }
                    Format::Human => e.exit(),
                }
            }
            let format = Format::detect(json_flag);
            output::print_clap_error(format, &e);
            std::process::exit(3);
        }
    };

    let ctx = Ctx::new(cli.json, cli.quiet);

    let result = match cli.command {
        Commands::Generate(args) => commands::generate::run(ctx, *args),
        Commands::Status { id, api_key } => commands::status::run(ctx, id, api_key),
        Commands::Download {
            id,
            output,
            api_key,
        } => commands::download::run(ctx, id, output, api_key),
        Commands::Cancel { id, api_key } => commands::cancel::run(ctx, id, api_key),
        Commands::CharacterSheet {
            input,
            output,
            style,
            angles,
        } => commands::character_sheet::run(ctx, input, output, style, angles),
        Commands::AudioToVideo {
            input,
            output,
            background,
            height,
        } => commands::audio_to_video::run(ctx, input, output, background, height),
        Commands::Models => commands::models::run(ctx),
        Commands::Doctor => {
            config::load().and_then(|cfg| commands::doctor::run(ctx, &cfg))
        }
        Commands::AgentInfo => {
            commands::agent_info::run();
            Ok(())
        }
        Commands::Skill { action } => match action {
            SkillAction::Install => commands::skill::install(ctx),
            SkillAction::Status => commands::skill::status(ctx),
        },
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                config::load().and_then(|cfg| commands::config::show(ctx, &cfg))
            }
            ConfigAction::Path => commands::config::path(ctx),
            ConfigAction::Set { key, value } => commands::config::set(ctx, key, value),
            ConfigAction::Unset { key } => commands::config::unset(ctx, key),
        },
        Commands::Update { check } => {
            config::load().and_then(|cfg| commands::update::run(ctx, check, &cfg))
        }
    };

    if let Err(e) = result {
        output::print_error(ctx.format, &e);
        std::process::exit(e.exit_code());
    }
}
