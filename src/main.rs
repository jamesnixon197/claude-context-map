mod analyse;
mod capture;
mod cli;
mod config;
mod model;
mod project;
mod render;
mod storage;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use model::{KindFilter, ReportOptions};
use std::io::IsTerminal;

const FALLBACK_TERMINAL_WIDTH: usize = 80;

struct ReportFlags {
    all: bool,
    kind: Vec<String>,
    top: Option<usize>,
    detail: bool,
}

fn resolve_report_options(flags: ReportFlags, config: &config::CcmapConfig) -> ReportOptions {
    let stdout_is_terminal = std::io::stdout().is_terminal();
    let color_disabled = std::env::var_os("NO_COLOR").is_some();
    let enabled = stdout_is_terminal && !color_disabled;

    let linear_base = std::env::var("LINEAR_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| config.links.linear.clone());

    let kinds = flags
        .kind
        .iter()
        .filter_map(|k| KindFilter::from_str_opt(k))
        .collect();

    ReportOptions {
        all: flags.all,
        use_color: enabled,
        use_links: enabled,
        terminal_width: terminal_width().unwrap_or(FALLBACK_TERMINAL_WIDTH),
        linear_base,
        kinds,
        top: flags.top,
        detail: flags.detail,
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
        Command::Analyse {
            path,
            all,
            kind,
            top,
            detail,
        } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            let analysis = analyse::analyse_file(&path, &config)?;
            let options = resolve_report_options(
                ReportFlags {
                    all,
                    kind,
                    top,
                    detail,
                },
                &config,
            );
            analyse::print_analysis(&analysis, &options);
        }
        Command::Latest {
            all,
            kind,
            top,
            detail,
        } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            match storage.latest_session_file()? {
                Some(path) => {
                    let analysis = analyse::analyse_file(&path, &config)?;
                    let options = resolve_report_options(
                        ReportFlags {
                            all,
                            kind,
                            top,
                            detail,
                        },
                        &config,
                    );
                    analyse::print_analysis(&analysis, &options);
                }
                None => println!("No sessions captured yet."),
            }
        }
        Command::Show { n } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            match storage.latest_session_file()? {
                Some(path) => {
                    let analysis = analyse::analyse_file(&path, &config)?;
                    let options = resolve_report_options(
                        ReportFlags {
                            all: true,
                            kind: Vec::new(),
                            top: None,
                            detail: false,
                        },
                        &config,
                    );
                    analyse::print_source_detail(&analysis, n, &options)?;
                }
                None => println!("No sessions captured yet."),
            }
        }
        Command::Digest {
            session,
            for_injection,
        } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            if !config.digest.enabled {
                return Ok(());
            }

            let target = match session {
                Some(path) => Some(path),
                None => {
                    let current = std::env::var("CLAUDE_CODE_SESSION_ID").ok();
                    storage.previous_substantive_session(
                        current.as_deref(),
                        config.digest.min_events,
                    )?
                }
            };

            let Some(path) = target else {
                return Ok(());
            };

            let analysis = analyse::analyse_file(&path, &config)?;
            if !analyse::has_signal(&analysis, config.digest.dominant_share_threshold) {
                return Ok(());
            }

            let body = analyse::digest_body(&analysis);
            if for_injection {
                print!("{}", analyse::wrap_for_injection(&body));
            } else {
                println!("{body}");
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
        Command::Graph { path } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            let target = match path {
                Some(path) => Some(path),
                None => storage.latest_session_file()?,
            };

            match target {
                Some(session_path) => {
                    let output_path = render::write_graph(&storage, &config, &session_path)?;
                    println!("Graph written: {}", output_path.display());
                }
                None => println!("No sessions captured yet."),
            }
        }
    }

    Ok(())
}
