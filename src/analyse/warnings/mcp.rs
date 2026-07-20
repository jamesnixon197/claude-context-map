use crate::analyse::warnings::helpers::describe_event;
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextSourceKind, ContextWarning, WarningSeverity};

pub fn warn_large_mcp_responses(
    events: &[ContextEvent],
    config: &CcmapConfig,
) -> Vec<ContextWarning> {
    let threshold = config.warning_rules.large_mcp_response_token_threshold;

    events
        .iter()
        .filter(|event| matches!(event.source_kind, ContextSourceKind::McpTool { .. }))
        .filter(|event| event.approx_tokens > threshold)
        .map(|event| ContextWarning {
            severity: WarningSeverity::Medium,
            title: format!("Large MCP response detected: {}", describe_event(event)),
            detail: format!(
                "The MCP tool call '{}' returned approximately {} tokens, which may be too large for effective processing.",
                describe_event(event),
                event.approx_tokens
            ),
        })
        .collect()
}
