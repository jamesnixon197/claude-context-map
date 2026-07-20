use crate::analyse::warnings::helpers::{describe_event, is_configured_generated_path};
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextWarning, WarningSeverity};

pub fn warn_generated_paths(events: &[ContextEvent], config: &CcmapConfig) -> Vec<ContextWarning> {
    events
        .iter()
        .filter(|event| {
            event
                .path
                .as_deref()
                .is_some_and(|path| is_configured_generated_path(path, config))
        })
        .map(|event| ContextWarning {
            severity: WarningSeverity::Medium,
            title: format!("Generated/dependency path observed: {}", describe_event(event)),
            detail: format!(
                "The context source '{}' is under a generated or dependency path, which may be low-signal unless directly relevant.",
                describe_event(event)
            ),
        })
        .collect()
}
