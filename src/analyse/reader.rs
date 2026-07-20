use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn read_jsonl_events(path: &Path) -> Result<Vec<Value>> {
    let content = fs::read_to_string(path)?;

    let mut raw_events = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line)?;
        raw_events.push(value);
    }

    if raw_events.is_empty() {
        return Err(anyhow!("Session file contains no events"));
    }

    Ok(raw_events)
}
