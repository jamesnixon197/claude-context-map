use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextSourceKind, ContextWarning, WarningSeverity};
use std::collections::HashMap;

pub fn warn_repeated_reads(events: &[ContextEvent], config: &CcmapConfig) -> Vec<ContextWarning> {
    let threshold = config.warning_rules.repeated_read_threshold;

    let mut read_counts: HashMap<&str, usize> = HashMap::new();

    for event in events {
        if event.source_kind != ContextSourceKind::FileRead {
            continue;
        }

        if let Some(path) = event.path.as_deref() {
            *read_counts.entry(path).or_insert(0) += 1;
        }
    }

    let mut paths: Vec<_> = read_counts
        .into_iter()
        .filter(|(_, count)| *count > threshold)
        .collect();

    paths.sort_by(|a, b| a.0.cmp(b.0));

    paths
        .into_iter()
        .map(|(path, count)| ContextWarning {
            severity: WarningSeverity::Low,
            title: format!("Repeated file read detected: {}", path),
            detail: format!(
                "'{}' was read {} times in this session. This may suggest inefficient context navigation, though repeated reads of a central file can be expected.",
                path, count
            ),
        })
        .collect()
}
