use crate::analyse::warnings::helpers::{describe_event, is_configured_lockfile};
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextSourceKind, ContextWarning, WarningSeverity};

pub fn warn_lockfile_reads(events: &[ContextEvent], config: &CcmapConfig) -> Vec<ContextWarning> {
    events
        .iter()
        .filter(|event| event.source_kind == ContextSourceKind::FileRead)
        .filter(|event| {
            event
                .path
                .as_deref()
                .is_some_and(|path| is_configured_lockfile(path, config))
        })
        .map(|event| ContextWarning {
            severity: WarningSeverity::High,
            title: format!("Lockfile read detected: {}", describe_event(event)),
            detail: format!(
                "The context source '{}' is a lockfile, which is often low-signal unless the task involves dependency resolution.",
                describe_event(event)
            ),
        })
        .collect()
}
