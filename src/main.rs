mod analyse;
mod capture;
mod cli;
mod config;
mod model;
mod project;
mod storage;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use model::ReportOptions;
use std::io::IsTerminal;

const FALLBACK_TERMINAL_WIDTH: usize = 80;

fn resolve_report_options(all: bool) -> ReportOptions {
    let stdout_is_terminal = std::io::stdout().is_terminal();
    let color_disabled = std::env::var_os("NO_COLOR").is_some();

    ReportOptions {
        all,
        use_color: stdout_is_terminal && !color_disabled,
        terminal_width: terminal_width().unwrap_or(FALLBACK_TERMINAL_WIDTH),
    }
}

fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|width| *width > 0)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);

            config::init_project(&project, &storage)?;

            println!("Claude Context Map initialised.");
            println!("Project: {}", project.name);
            println!("Storage: {}", storage.base_dir.display());
            println!("Settings: {}", storage.settings_file.display());
        }
        Command::Capture => {
            capture::capture_from_stdin()?;
        }
        Command::Analyse { path, all } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            let analysis = analyse::analyse_file(&path, &config)?;

            analyse::print_analysis(&analysis, resolve_report_options(all));
        }
        Command::Latest { all } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            match storage.latest_session_file()? {
                Some(path) => {
                    let analysis = analyse::analyse_file(&path, &config)?;
                    analyse::print_analysis(&analysis, resolve_report_options(all));
                }
                None => println!("No sessions captured yet."),
            }
        }
        Command::History => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);

            let sessions = storage.session_files()?;

            if sessions.is_empty() {
                println!("No sessions captured yet.");
            } else {
                for session in sessions {
                    println!("{}", session.display());
                }
            }
        }
        Command::Doctor => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);

            println!("Project root: {}", project.root.display());
            println!("Project name: {}", project.name);
            println!("Project id:   {}", project.id);
            println!("Config file:  {}", storage.config_file.display());
            println!("Config exists: {}", storage.config_file.exists());
        }
    }

    Ok(())
}
