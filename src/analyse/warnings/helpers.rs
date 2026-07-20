use crate::config::CcmapConfig;
use crate::model::ContextEvent;

pub fn describe_event(event: &ContextEvent) -> String {
    if let Some(path) = &event.path {
        return path.clone();
    }

    if let Some(command) = &event.command {
        return format!("Command: {}", command);
    }

    if let Some(source_label) = &event.source_label {
        return source_label.clone();
    }

    event
        .tool_name
        .clone()
        .unwrap_or_else(|| format!("{:?}", event.source_kind))
}

pub fn is_configured_lockfile(path: &str, config: &CcmapConfig) -> bool {
    config
        .warning_rules
        .lockfile_names
        .iter()
        .any(|lockfile_name| path.ends_with(lockfile_name.as_str()))
}

pub fn is_configured_generated_path(path: &str, config: &CcmapConfig) -> bool {
    config
        .warning_rules
        .generated_path_segments
        .iter()
        .any(|segment| path.contains(segment.as_str()))
}
