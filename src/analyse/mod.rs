mod classify;
mod estimate;
mod mcp;
mod normalise;
mod reader;
mod report;
mod summary;
mod warnings;

use crate::config::CcmapConfig;
use crate::model::SessionAnalysis;
use anyhow::Result;
use std::path::Path;

pub use report::print_analysis;

pub fn analyse_file(path: &Path, config: &CcmapConfig) -> Result<SessionAnalysis> {
    let raw_events = reader::read_jsonl_events(path)?;
    let events = normalise::normalise_events(&raw_events);

    Ok(summary::build_analysis(events, config))
}
