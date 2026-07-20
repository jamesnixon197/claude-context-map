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

        // Normally one JSON object per line. Tolerate lines carrying more than
        // one object, which a pre-fix concurrent capture could produce by
        // gluing two records together without a separating newline.
        for value in serde_json::Deserializer::from_str(line).into_iter::<Value>() {
            raw_events.push(value?);
        }
    }

    if raw_events.is_empty() {
        return Err(anyhow!("Session file contains no events"));
    }

    Ok(raw_events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("ccmap-reader-test-{name}.jsonl"));
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn reads_one_object_per_line() {
        let path = write_temp("clean", "{\"a\":1}\n{\"b\":2}\n");
        let events = read_jsonl_events(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["a"], 1);
        assert_eq!(events[1]["b"], 2);
    }

    #[test]
    fn recovers_two_objects_glued_onto_one_line() {
        // Reproduces the concurrent-append corruption: two records with no
        // separating newline, followed by the displaced blank line.
        let path = write_temp("glued", "{\"a\":1}{\"b\":2}\n\n{\"c\":3}\n");
        let events = read_jsonl_events(&path).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["a"], 1);
        assert_eq!(events[1]["b"], 2);
        assert_eq!(events[2]["c"], 3);
    }

    #[test]
    fn errors_on_genuinely_malformed_line() {
        let path = write_temp("broken", "{\"a\":1}\n{not json}\n");
        assert!(read_jsonl_events(&path).is_err());
    }
}
