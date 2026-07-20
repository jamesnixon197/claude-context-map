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
        Command::Analyse { path } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            let analysis = analyse::analyse_file(&path, &config)?;

            analyse::print_analysis(&analysis);
        }
        Command::Latest => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            match storage.latest_session_file()? {
                Some(path) => {
                    let analysis = analyse::analyse_file(&path, &config)?;
                    analyse::print_analysis(&analysis);
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
