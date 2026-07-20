mod classify;
mod estimate;
mod normalise;
mod reader;
mod summary;
mod warnings;

use crate::config::CcmapConfig;
use crate::model::SessionAnalysis;
use anyhow::Result;
use std::path::Path;

pub fn analyse_file(path: &Path, config: &CcmapConfig) -> Result<SessionAnalysis> {
    let raw_events = reader::read_jsonl_events(path)?;

    let events = raw_events
        .iter()
        .map(normalise::normalise_event)
        .collect::<Vec<_>>();

    Ok(summary::build_analysis(events, config))
}
