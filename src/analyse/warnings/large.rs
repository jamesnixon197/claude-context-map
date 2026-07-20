use crate::analyse::warnings::helpers::describe_event;
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextWarning, WarningSeverity};

pub fn warn_large_context_sources(
    events: &[ContextEvent],
    config: &CcmapConfig,
) -> Vec<ContextWarning> {
    let threshold = config.warning_rules.large_context_token_threshold;

    events
        .iter()
        .filter(|event| event.approx_tokens > threshold)
        .map(|event| ContextWarning {
            severity: WarningSeverity::Medium,
            title: format!("Large context source detected: {}", describe_event(event)),
            detail: format!(
                "The context source '{}' has approximately {} tokens, which may be too large for effective processing.",
                describe_event(event),
                event.approx_tokens
            ),
        })
        .collect()
}
