mod helpers;
mod large;
mod lockfiles;

use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextWarning};

pub fn build_warnings(events: &[ContextEvent], config: &CcmapConfig) -> Vec<ContextWarning> {
    let mut warnings = Vec::new();

    warnings.extend(large::warn_large_context_sources(events, config));
    warnings.extend(lockfiles::warn_lockfile_reads(events, config));

    warnings
}
