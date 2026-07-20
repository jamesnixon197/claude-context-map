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
        #[arg(long, value_name = "KIND")]
        kind: Vec<String>,
        #[arg(long, value_name = "N")]
        top: Option<usize>,
        #[arg(long)]
        detail: bool,
    },
    Latest {
        #[arg(short, long)]
        all: bool,
        #[arg(long, value_name = "KIND")]
        kind: Vec<String>,
        #[arg(long, value_name = "N")]
        top: Option<usize>,
        #[arg(long)]
        detail: bool,
    },
    Show {
        n: usize,
    },
    Digest {
        #[arg(long, value_name = "PATH")]
        session: Option<PathBuf>,
        #[arg(long)]
        for_injection: bool,
    },
    History,
    Doctor,
    Graph {
        path: Option<PathBuf>,
    },
}
