use crate::project::Project;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Storage {
    pub base_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub reports_dir: PathBuf,
    pub project_file: PathBuf,
    pub config_file: PathBuf,
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
            config_file: base_dir.join("config.toml"),
            settings_file: claude_dir.join("settings.local.json"),
        }
    }

    pub fn create_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.sessions_dir)?;
        fs::create_dir_all(&self.reports_dir)?;
        Ok(())
    }

    pub fn session_files(&self) -> Result<Vec<PathBuf>> {
        if !self.sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.sessions_dir)? {
            let path = entry?.path();

            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                sessions.push(path);
            }
        }

        sessions.sort();

        Ok(sessions)
    }

    pub fn latest_session_file(&self) -> Result<Option<PathBuf>> {
        let mut sessions = self.session_files()?;

        let modified_times = sessions
            .iter()
            .map(|path| fs::metadata(path).and_then(|meta| meta.modified()))
            .collect::<std::io::Result<Vec<SystemTime>>>()?;

        let mut sessions_with_times: Vec<_> = sessions.drain(..).zip(modified_times).collect();

        sessions_with_times.sort_by_key(|(_, modified)| *modified);

        Ok(sessions_with_times.pop().map(|(path, _)| path))
    }
}
