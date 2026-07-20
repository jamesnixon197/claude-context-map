use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ccmap")]
#[command(about = "Claude Code context observability")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Init,
    Capture,
    Analyse {
        path: PathBuf,
        #[arg(short, long)]
        all: bool,
    },
    Latest {
        #[arg(short, long)]
        all: bool,
    },
    History,
    Doctor,
}
