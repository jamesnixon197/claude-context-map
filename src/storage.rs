use crate::project::Project;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Storage {
    pub base_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub reports_dir: PathBuf,
    pub project_file: PathBuf,
    pub settings_file: PathBuf,
}

impl Storage {
    pub fn for_project(project: &Project) -> Self {
        let claude_dir = project.root.join(".claude");
        let base_dir = claude_dir.join("context-map");

        Self {
            base_dir: base_dir.clone(),
            sessions_dir: base_dir.join("sessions"),
            reports_dir: base_dir.join("reports"),
            project_file: base_dir.join("project.json"),
            settings_file: claude_dir.join("settings.local.json"),
        }
    }

    pub fn create_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.sessions_dir)?;
        fs::create_dir_all(&self.reports_dir)?;
        Ok(())
    }
}
