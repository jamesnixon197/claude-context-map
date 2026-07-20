use crate::analyse::warnings::helpers::describe_event;
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextSourceKind, ContextWarning, WarningSeverity};

pub fn warn_large_shell_output(
    events: &[ContextEvent],
    config: &CcmapConfig,
) -> Vec<ContextWarning> {
    let threshold = config.warning_rules.large_shell_output_token_threshold;

    events
        .iter()
        .filter(|event| event.source_kind == ContextSourceKind::ShellOutput)
        .filter(|event| event.approx_tokens > threshold)
        .map(|event| ContextWarning {
            severity: WarningSeverity::Medium,
            title: format!("Large shell output detected: {}", describe_event(event)),
            detail: format!(
                "The command '{}' produced approximately {} tokens of output, which can dominate context, especially for test suites, build logs, or stack traces.",
                describe_event(event),
                event.approx_tokens
            ),
        })
        .collect()
}
