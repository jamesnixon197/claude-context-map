use clap::{Parser, Subcommand};

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
    Latest,
    History,
    Doctor,
}
