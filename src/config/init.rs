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
    let settings = json!({
        "hooks": {
            "SessionStart": [
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
        }
    });

    if let Some(parent) = storage.settings_file.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &storage.settings_file,
        serde_json::to_string_pretty(&settings)?,
    )?;

    Ok(())
}
