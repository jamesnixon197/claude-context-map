use crate::config::defaults::write_default_config_if_missing;
use crate::project::Project;
use crate::storage::Storage;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::fs;

pub fn init_project(project: &Project, storage: &Storage) -> Result<()> {
    storage.create_dirs()?;

    write_project_file(project, storage)?;
    write_default_config_if_missing(storage)?;
    write_local_settings(storage)?;

    Ok(())
}

fn write_project_file(project: &Project, storage: &Storage) -> Result<()> {
    let project_json = json!({
        "name": project.name,
        "project_id": project.id,
        "created_at": Utc::now().to_rfc3339(),
        "mode": "safe"
    });

    fs::write(
        &storage.project_file,
        serde_json::to_string_pretty(&project_json)?,
    )?;

    Ok(())
}

fn write_local_settings(storage: &Storage) -> Result<()> {
    let hooks = json!({
        "SessionStart": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            },
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap digest --for-injection"
                    }
                ]
            }
        ],
        "InstructionsLoaded": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "PostToolUse": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "PostToolBatch": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "SubagentStart": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "SubagentStop": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "Stop": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ]
    });

    let existing_content = fs::read_to_string(&storage.settings_file).ok();
    let merged = merge_settings(existing_content.as_deref(), &hooks)?;

    if let Some(parent) = storage.settings_file.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &storage.settings_file,
        serde_json::to_string_pretty(&merged)?,
    )?;

    Ok(())
}

pub(crate) fn merge_settings(
    existing: Option<&str>,
    hooks_to_ensure: &serde_json::Value,
) -> Result<serde_json::Value> {
    let mut root = match existing
        .and_then(|content| serde_json::from_str::<serde_json::Value>(content).ok())
    {
        Some(value) if value.is_object() => value,
        _ => serde_json::json!({}),
    };

    if !root
        .get("hooks")
        .map(|hooks| hooks.is_object())
        .unwrap_or(false)
    {
        root["hooks"] = serde_json::json!({});
    }

    let hooks_map = root["hooks"].as_object_mut().expect("just ensured object");
    let ensure_map = hooks_to_ensure
        .as_object()
        .expect("hooks_to_ensure must be a JSON object");

    for (event_name, ccmap_entries) in ensure_map {
        let ccmap_entries = ccmap_entries
            .as_array()
            .expect("hooks_to_ensure entries must be arrays");

        match hooks_map.get_mut(event_name) {
            None => {
                hooks_map.insert(
                    event_name.clone(),
                    serde_json::Value::Array(ccmap_entries.clone()),
                );
            }
            Some(existing_value) => {
                let Some(existing_array) = existing_value.as_array_mut() else {
                    // Malformed/unexpected shape — leave it alone rather than guessing.
                    continue;
                };

                for ccmap_entry in ccmap_entries {
                    let ccmap_commands = commands_in_entry(ccmap_entry);
                    let already_present = ccmap_commands.iter().all(|cmd| {
                        existing_array
                            .iter()
                            .any(|e| commands_in_entry(e).contains(cmd))
                    });

                    if !already_present {
                        existing_array.push(ccmap_entry.clone());
                    }
                }
            }
        }
    }

    Ok(root)
}

fn commands_in_entry(entry: &serde_json::Value) -> Vec<String> {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks
                .iter()
                .filter_map(|h| h.get("command").and_then(|c| c.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ccmap_hooks() -> serde_json::Value {
        json!({
            "SessionStart": [
                { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] },
                { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap digest --for-injection" } ] }
            ],
            "Stop": [
                { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] }
            ]
        })
    }

    #[test]
    fn merge_settings_with_no_existing_file_writes_ccmap_hooks_fresh() {
        let result = merge_settings(None, &ccmap_hooks()).unwrap();
        assert_eq!(result, json!({ "hooks": ccmap_hooks() }));
    }

    #[test]
    fn merge_settings_preserves_a_hook_event_ccmap_does_not_manage() {
        let existing = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "", "hooks": [ { "type": "command", "command": "my-custom-check" } ] }
                ]
            }
        })
        .to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        assert_eq!(
            result["hooks"]["PreToolUse"],
            json!([{ "matcher": "", "hooks": [ { "type": "command", "command": "my-custom-check" } ] }])
        );
        assert_eq!(
            result["hooks"]["SessionStart"],
            ccmap_hooks()["SessionStart"]
        );
    }

    #[test]
    fn merge_settings_is_idempotent_when_ccmap_hooks_already_present() {
        let existing = json!({ "hooks": ccmap_hooks() }).to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        assert_eq!(
            result["hooks"]["SessionStart"].as_array().unwrap().len(),
            2,
            "re-running the merge must not duplicate entries"
        );
        assert_eq!(
            result["hooks"]["Stop"].as_array().unwrap().len(),
            1,
            "re-running the merge must not duplicate entries"
        );
    }

    #[test]
    fn merge_settings_adds_only_the_missing_hook_to_an_old_project() {
        // Simulates an old project generated before the digest hook existed:
        // SessionStart has only the "ccmap capture" entry.
        let existing = json!({
            "hooks": {
                "SessionStart": [
                    { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] }
                ],
                "Stop": [
                    { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] }
                ]
            }
        })
        .to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        let session_start = result["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 2, "should add the missing digest hook");
        let commands: Vec<&str> = session_start
            .iter()
            .flat_map(|entry| entry["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert!(commands.contains(&"ccmap capture"));
        assert!(commands.contains(&"ccmap digest --for-injection"));

        assert_eq!(
            result["hooks"]["Stop"].as_array().unwrap().len(),
            1,
            "Stop already had ccmap's only entry, should not duplicate"
        );
    }

    #[test]
    fn merge_settings_leaves_a_non_array_hook_event_alone() {
        let existing = json!({
            "hooks": {
                "SessionStart": "not-an-array-somehow"
            }
        })
        .to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        assert_eq!(
            result["hooks"]["SessionStart"],
            json!("not-an-array-somehow")
        );
    }

    #[test]
    fn merge_settings_falls_back_to_fresh_output_on_unparseable_existing_content() {
        let result = merge_settings(Some("{ this is not valid json"), &ccmap_hooks()).unwrap();
        assert_eq!(result, json!({ "hooks": ccmap_hooks() }));
    }

    #[test]
    fn write_local_settings_includes_the_digest_hook_on_a_fresh_write() {
        let dir = std::env::temp_dir().join(format!(
            "ccmap-init-test-{}-{}",
            std::process::id(),
            "fresh"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let storage = crate::storage::Storage {
            base_dir: dir.clone(),
            sessions_dir: dir.join("sessions"),
            reports_dir: dir.join("reports"),
            project_file: dir.join("project.json"),
            config_file: dir.join("config.toml"),
            settings_file: dir.join("settings.local.json"),
        };

        write_local_settings(&storage).unwrap();

        let content = std::fs::read_to_string(&storage.settings_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let session_start = parsed["hooks"]["SessionStart"].as_array().unwrap();
        let commands: Vec<&str> = session_start
            .iter()
            .flat_map(|entry| entry["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert!(commands.contains(&"ccmap capture"));
        assert!(commands.contains(&"ccmap digest --for-injection"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_local_settings_is_safe_to_run_twice_without_duplicating_hooks() {
        let dir = std::env::temp_dir().join(format!(
            "ccmap-init-test-{}-{}",
            std::process::id(),
            "twice"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let storage = crate::storage::Storage {
            base_dir: dir.clone(),
            sessions_dir: dir.join("sessions"),
            reports_dir: dir.join("reports"),
            project_file: dir.join("project.json"),
            config_file: dir.join("config.toml"),
            settings_file: dir.join("settings.local.json"),
        };

        write_local_settings(&storage).unwrap();
        write_local_settings(&storage).unwrap();

        let content = std::fs::read_to_string(&storage.settings_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let session_start = parsed["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(
            session_start.len(),
            2,
            "running init twice must not duplicate the SessionStart hooks"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
