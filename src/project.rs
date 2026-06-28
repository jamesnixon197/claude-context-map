use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub id: String,
}

pub fn find_project() -> Result<Project> {
    let cwd = env::current_dir()?;
    let root = find_project_root(&cwd).ok_or_else(|| {
        anyhow!("Could not find project root. Run ccmap inside a git or Claude project.")
    })?;

    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown-project")
        .to_string();

    let id = hash_path(&root);

    Ok(Project { root, name, id })
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);

    while let Some(path) = current {
        if path.join(".git").exists() || path.join(".claude").exists() {
            return Some(path.to_path_buf());
        }

        current = path.parent();
    }

    None
}

fn hash_path(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());

    let hash = hasher.finalize();
    hex::encode(hash)[0..12].to_string()
}
