use super::report::format_count;
use crate::model::{SessionAnalysis, WarningSeverity};

pub fn has_signal(analysis: &SessionAnalysis, dominant_share_threshold: f64) -> bool {
    if !analysis.warnings.is_empty() {
        return true;
    }
    let total: usize = analysis.context_map.iter().map(|s| s.approx_tokens).sum();
    if total == 0 {
        return false;
    }
    analysis
        .context_map
        .iter()
        .map(|s| s.approx_tokens)
        .max()
        .map(|top| top as f64 / total as f64 >= dominant_share_threshold)
        .unwrap_or(false)
}

fn basename(label: &str) -> String {
    label.rsplit('/').next().unwrap_or(label).to_string()
}

fn count_severity(analysis: &SessionAnalysis) -> (usize, usize, usize) {
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;
    for warning in &analysis.warnings {
        match warning.severity {
            WarningSeverity::High => high += 1,
            WarningSeverity::Medium => medium += 1,
            WarningSeverity::Low => low += 1,
        }
    }
    (high, medium, low)
}

pub fn digest_body(analysis: &SessionAnalysis) -> String {
    let total: usize = analysis
        .context_map
        .iter()
        .map(|s| s.approx_tokens)
        .sum::<usize>()
        .max(1);

    let short_id = analysis
        .session_id
        .split('-')
        .next()
        .unwrap_or(&analysis.session_id);

    let top: Vec<String> = analysis
        .context_map
        .iter()
        .take(3)
        .map(|source| {
            let pct = (source.approx_tokens as f64 / total as f64 * 100.0).round() as usize;
            let occ = if source.occurrences > 1 {
                format!(" (read {}×)", source.occurrences)
            } else {
                String::new()
            };
            format!("{} {}%{}", basename(&source.source_label), pct, occ)
        })
        .collect();

    let (high, medium, low) = count_severity(analysis);
    let mut warn_parts = Vec::new();
    if high > 0 {
        warn_parts.push(format!("{high} high"));
    }
    if medium > 0 {
        warn_parts.push(format!("{medium} medium"));
    }
    if low > 0 {
        warn_parts.push(format!("{low} low"));
    }
    let warn_line = if warn_parts.is_empty() {
        "Warnings: none.".to_string()
    } else {
        format!("Warnings: {}.", warn_parts.join(", "))
    };

    format!(
        "ccmap — previous session ({}): ~{} tokens across {} sources.\nTop consumers: {}.\n{}",
        short_id,
        format_count(analysis.approx_context_tokens),
        analysis.context_map.len(),
        top.join(", "),
        warn_line,
    )
}

pub fn wrap_for_injection(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }
    format!(
        "<ccmap-previous-session-digest>\nContext usage from the user's previous session in this project:\n{body}\n\nIf relevant to how this session starts, you may briefly mention where the user's context went last time and offer to work in a way that avoids it. Don't force it if the user is already focused on a task.\n</ccmap-previous-session-digest>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ContextSourceKind, ContextSourceSummary, ContextWarning};

    fn analysis(tokens: usize, sources: Vec<(usize, &str)>, warnings: usize) -> SessionAnalysis {
        SessionAnalysis {
            session_id: "01e1536f-aaaa".to_string(),
            instruction_files_loaded: 0,
            files_read: 0,
            file_searches: 0,
            file_path_lists: 0,
            bash_commands: 0,
            files_edited: 0,
            files_written: 0,
            subagents: 0,
            approx_context_tokens: tokens,
            context_map: sources
                .into_iter()
                .map(|(t, label)| ContextSourceSummary {
                    source_kind: ContextSourceKind::FileRead,
                    source_label: label.to_string(),
                    occurrences: 1,
                    approx_tokens: t,
                    trigger_reasons: Vec::new(),
                })
                .collect(),
            events: Vec::new(),
            warnings: (0..warnings)
                .map(|_| ContextWarning {
                    severity: WarningSeverity::Medium,
                    title: "Large context source detected: /x/y.txt".to_string(),
                    detail: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn has_signal_true_when_a_warning_exists() {
        let a = analysis(1000, vec![(100, "a")], 1);
        assert!(has_signal(&a, 0.25));
    }

    #[test]
    fn has_signal_true_when_top_source_dominates() {
        let a = analysis(1000, vec![(400, "big"), (100, "small")], 0);
        assert!(has_signal(&a, 0.25));
    }

    #[test]
    fn has_signal_false_when_clean_and_no_dominant() {
        let a = analysis(
            1000,
            vec![(100, "a"), (100, "b"), (100, "c"), (100, "d"), (100, "e")],
            0,
        );
        assert!(!has_signal(&a, 0.25));
    }

    #[test]
    fn digest_body_lists_top_consumers_as_basenames_with_share() {
        let a = analysis(
            10_000,
            vec![
                (3200, "/Users/j/repo/scratchpad/sds_text.txt"),
                (600, "/Users/j/repo/src/ingress.test.ts"),
            ],
            2,
        );
        let body = digest_body(&a);
        assert!(body.contains("sds_text.txt"));
        assert!(!body.contains("/Users/"));
        assert!(body.contains('%'));
        assert!(body.contains("2 medium"));
    }

    #[test]
    fn wrap_for_injection_empty_body_is_empty() {
        assert_eq!(wrap_for_injection(""), "");
    }

    #[test]
    fn wrap_for_injection_wraps_non_empty() {
        let out = wrap_for_injection("hello");
        assert!(out.contains("<ccmap-previous-session-digest>"));
        assert!(out.contains("hello"));
    }
}
