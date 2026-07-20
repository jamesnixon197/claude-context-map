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

    let mut line = serde_json::to_string(event)?;
    line.push('\n');

    // ccmap capture runs as a Claude Code hook: many short-lived processes
    // append to the same session file concurrently. O_APPEND makes a single
    // write syscall land atomically at end-of-file, so emit the record and its
    // newline in one write_all. Splitting them (e.g. writeln!) lets two
    // simultaneous captures interleave and glue two objects onto one line.
    file.write_all(line.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn concurrent_appends_never_glue_two_objects_onto_one_line() {
        let path = std::env::temp_dir().join("ccmap-capture-concurrency-test.jsonl");
        let _ = std::fs::remove_file(&path);

        let writers = 16;
        let per_writer = 40;

        let handles: Vec<_> = (0..writers)
            .map(|writer| {
                let path = path.clone();
                thread::spawn(move || {
                    for seq in 0..per_writer {
                        let event = serde_json::json!({ "writer": writer, "seq": seq });
                        append_json_line(&path, &event).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let contents = std::fs::read_to_string(&path).unwrap();
        let mut lines = 0;
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            lines += 1;
            serde_json::from_str::<Value>(line).unwrap_or_else(|error| {
                panic!("line is not exactly one JSON object ({error}): {line}");
            });
        }
        assert_eq!(lines, writers * per_writer);

        let _ = std::fs::remove_file(&path);
    }
}
