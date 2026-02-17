mod cli;
mod client;
mod config;
mod core;
mod error;
mod format;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};

fn main() -> ExitCode {
    // Preprocess argv: if first arg is not a known subcommand or flag, insert "analyze"
    let mut args: Vec<String> = std::env::args().collect();
    if let Some(first) = args.get(1) {
        if !matches!(
            first.as_str(),
            "config" | "analyze" | "--help" | "-h" | "--version" | "-V"
        ) {
            args.insert(1, "analyze".to_string());
        }
    }

    let cli = match Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e) => {
            e.print().ok();
            return ExitCode::from(if e.use_stderr() { 2 } else { 0 });
        }
    };

    let result = match cli.command {
        Commands::Analyze(analyze_args) => cli::handle_analyze(analyze_args),
        Commands::Config { action } => cli::handle_config(action),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
