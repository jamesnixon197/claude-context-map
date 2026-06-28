use crate::project;
use crate::storage::Storage;
use anyhow::{Result, anyhow};
use chrono::Utc;
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::{Read, Write};

pub fn capture_from_stdin() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    if input.trim().is_empty() {
        return Err(anyhow!("No hook JSON received on stdin"));
    }

    let mut event: Value = serde_json::from_str(&input)?;

    add_capture_metadata(&mut event);

    let session_id = extract_session_id(&event);
    let project = project::find_project()?;
    let storage = Storage::for_project(&project);

    storage.create_dirs()?;

    let session_file = storage.sessions_dir.join(format!("{session_id}.jsonl"));

    append_json_line(&session_file, &event)?;

    Ok(())
}

fn add_capture_metadata(event: &mut Value) {
    event["ccmap_captured_at"] = Value::String(Utc::now().to_rfc3339());
}

fn extract_session_id(event: &Value) -> String {
    event
        .get("session_id")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown-session")
        .to_string()
}

fn append_json_line(path: &std::path::Path, event: &Value) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    let line = serde_json::to_string(event)?;

    writeln!(file, "{line}")?;

    Ok(())
}
