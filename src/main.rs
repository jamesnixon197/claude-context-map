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
        Command::Latest => {
            println!("latest not implemented yet");
        }
        Command::History => {
            println!("history not implemented yet");
        }
        Command::Doctor => {
            let project = project::find_project()?;
            println!("Project root: {}", project.root.display());
            println!("Project name: {}", project.name);
            println!("Project id:   {}", project.id);
        }
    }

    Ok(())
}
