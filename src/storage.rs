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

    pub fn previous_substantive_session(
        &self,
        current_id: Option<&str>,
        min_events: usize,
    ) -> Result<Option<PathBuf>> {
        let files = self.session_files()?;
        let mut entries: Vec<(String, String, usize, u64)> = Vec::new();
        for path in &files {
            let mtime = fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let path_str = path.to_string_lossy().to_string();
            entries.push((path_str, session_id_of(path), count_lines(path), mtime));
        }
        Ok(select_previous(&entries, current_id, min_events).map(PathBuf::from))
    }
}

fn select_previous(
    entries: &[(String, String, usize, u64)],
    current_id: Option<&str>,
    min_events: usize,
) -> Option<String> {
    let mut candidates: Vec<&(String, String, usize, u64)> = entries
        .iter()
        .filter(|(_, id, lines, _)| Some(id.as_str()) != current_id && *lines >= min_events)
        .collect();
    candidates.sort_by_key(|(_, _, _, mtime)| *mtime);
    candidates.last().map(|(path, _, _, _)| path.clone())
}

fn count_lines(path: &std::path::Path) -> usize {
    fs::read_to_string(path)
        .map(|content| content.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0)
}

fn session_id_of(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_previous_skips_current_and_trivial_picks_newest() {
        let entries = vec![
            ("cur.jsonl".to_string(), "CUR".to_string(), 50usize, 3u64),
            ("big.jsonl".to_string(), "BIG".to_string(), 40usize, 2u64),
            ("tiny.jsonl".to_string(), "TINY".to_string(), 2usize, 1u64),
        ];
        let pick = select_previous(&entries, Some("CUR"), 5);
        assert_eq!(pick.as_deref(), Some("big.jsonl"));
    }

    #[test]
    fn select_previous_returns_none_when_only_current_or_trivial() {
        let entries = vec![
            ("cur.jsonl".to_string(), "CUR".to_string(), 50usize, 2u64),
            ("tiny.jsonl".to_string(), "TINY".to_string(), 1usize, 1u64),
        ];
        assert_eq!(select_previous(&entries, Some("CUR"), 5), None);
    }

    #[test]
    fn select_previous_without_current_id_still_skips_trivial() {
        let entries = vec![
            ("a.jsonl".to_string(), "A".to_string(), 9usize, 2u64),
            ("b.jsonl".to_string(), "B".to_string(), 3usize, 3u64),
        ];
        assert_eq!(select_previous(&entries, None, 5).as_deref(), Some("a.jsonl"));
    }
}
