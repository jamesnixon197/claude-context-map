use crate::storage::Storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CcmapConfig {
    pub mode: CaptureMode,
    pub warning_rules: WarningRules,
    pub links: LinkConfig,
    pub digest: DigestConfig,
}

impl Default for CcmapConfig {
    fn default() -> Self {
        Self {
            mode: CaptureMode::default(),
            warning_rules: WarningRules::default(),
            links: LinkConfig::default(),
            digest: DigestConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkConfig {
    pub linear: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DigestConfig {
    pub enabled: bool,
    pub dominant_share_threshold: f64,
    pub min_events: usize,
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dominant_share_threshold: 0.25,
            min_events: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    #[default]
    Safe,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WarningRules {
    pub large_context_token_threshold: usize,
    pub repeated_read_threshold: usize,
    pub large_shell_output_token_threshold: usize,
    pub large_mcp_response_token_threshold: usize,
    pub lockfile_names: Vec<String>,
    pub generated_path_segments: Vec<String>,
}

impl Default for WarningRules {
    fn default() -> Self {
        Self {
            large_context_token_threshold: 4_000,
            repeated_read_threshold: 3,
            large_shell_output_token_threshold: 4_000,
            large_mcp_response_token_threshold: 4_000,
            lockfile_names: vec![
                "package-lock.json".to_string(),
                "pnpm-lock.yaml".to_string(),
                "yarn.lock".to_string(),
                "Cargo.lock".to_string(),
                "go.sum".to_string(),
            ],
            generated_path_segments: vec![
                "/dist/".to_string(),
                "/build/".to_string(),
                "/coverage/".to_string(),
                "/target/".to_string(),
                "/node_modules/".to_string(),
                "/.next/".to_string(),
            ],
        }
    }
}

pub fn load_config(storage: &Storage) -> Result<CcmapConfig> {
    if !storage.config_file.exists() {
        return Ok(CcmapConfig::default());
    }

    let content = fs::read_to_string(&storage.config_file)?;
    let config = toml::from_str(&content)?;

    Ok(config)
}

pub fn write_default_config_if_missing(storage: &Storage) -> Result<()> {
    if storage.config_file.exists() {
        return Ok(());
    }

    let config = CcmapConfig::default();
    let content = toml::to_string_pretty(&config)?;

    fs::write(&storage.config_file, content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_links_linear_base() {
        let toml = "mode = \"safe\"\n[links]\nlinear = \"https://linear.app/acme\"\n";
        let config: CcmapConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            config.links.linear.as_deref(),
            Some("https://linear.app/acme")
        );
    }

    #[test]
    fn config_defaults_links_to_none() {
        let config = CcmapConfig::default();
        assert_eq!(config.links.linear, None);
    }

    #[test]
    fn config_defaults_digest_thresholds() {
        let config = CcmapConfig::default();
        assert_eq!(config.digest.dominant_share_threshold, 0.25);
        assert_eq!(config.digest.min_events, 5);
    }

    #[test]
    fn config_parses_digest_overrides() {
        let toml = "mode = \"safe\"\n[digest]\ndominant_share_threshold = 0.4\nmin_events = 10\n";
        let config: CcmapConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.digest.dominant_share_threshold, 0.4);
        assert_eq!(config.digest.min_events, 10);
    }

    #[test]
    fn config_defaults_digest_enabled_to_true() {
        let config = CcmapConfig::default();
        assert!(config.digest.enabled);
    }

    #[test]
    fn config_parses_digest_enabled_override() {
        let toml = "mode = \"safe\"\n[digest]\nenabled = false\n";
        let config: CcmapConfig = toml::from_str(toml).unwrap();
        assert!(!config.digest.enabled);
    }
}
