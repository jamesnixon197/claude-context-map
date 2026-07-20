mod classify;
mod digest;
mod estimate;
mod mcp;
mod normalise;
mod reader;
mod report;
mod status_line;
mod summary;
mod warnings;

use crate::config::CcmapConfig;
use crate::model::SessionAnalysis;
use anyhow::Result;
use std::path::Path;

pub use digest::{digest_body, has_signal, wrap_for_injection};
pub use report::{print_analysis, print_source_detail};
pub use status_line::status_line;

pub fn analyse_file(path: &Path, config: &CcmapConfig) -> Result<SessionAnalysis> {
    let raw_events = reader::read_jsonl_events(path)?;
    let events = normalise::normalise_events(&raw_events);

    Ok(summary::build_analysis(events, config))
}
