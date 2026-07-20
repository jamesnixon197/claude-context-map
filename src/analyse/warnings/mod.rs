mod generated;
mod helpers;
mod large;
mod lockfiles;
mod mcp;
mod repeated;
mod shell;

use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextWarning};

pub fn build_warnings(events: &[ContextEvent], config: &CcmapConfig) -> Vec<ContextWarning> {
    let mut warnings = Vec::new();

    warnings.extend(large::warn_large_context_sources(events, config));
    warnings.extend(lockfiles::warn_lockfile_reads(events, config));
    warnings.extend(generated::warn_generated_paths(events, config));
    warnings.extend(repeated::warn_repeated_reads(events, config));
    warnings.extend(shell::warn_large_shell_output(events, config));
    warnings.extend(mcp::warn_large_mcp_responses(events, config));

    warnings
}
