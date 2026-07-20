use super::report::kind_tag;
use crate::model::{SessionAnalysis, WarningSeverity};
use owo_colors::{OwoColorize, Style};

const GAUGE_GLYPHS: [char; 5] = ['░', '◔', '◑', '◕', '●'];

fn gauge_glyph(share: f64) -> char {
    let clamped = share.clamp(0.0, 1.0);
    let index = (clamped * (GAUGE_GLYPHS.len() - 1) as f64).round() as usize;
    GAUGE_GLYPHS[index.min(GAUGE_GLYPHS.len() - 1)]
}

fn worst_severity(analysis: &SessionAnalysis) -> Option<WarningSeverity> {
    analysis
        .warnings
        .iter()
        .map(|w| w.severity.clone())
        .max_by_key(|severity| match severity {
            WarningSeverity::Low => 0,
            WarningSeverity::Medium => 1,
            WarningSeverity::High => 2,
        })
}

fn severity_style(severity: Option<&WarningSeverity>) -> Style {
    match severity {
        Some(WarningSeverity::High) => Style::new().red(),
        Some(WarningSeverity::Medium) | Some(WarningSeverity::Low) => Style::new().yellow(),
        None => Style::new().green(),
    }
}

pub fn status_line(analysis: &SessionAnalysis, use_color: bool) -> String {
    let total: usize = analysis.context_map.iter().map(|s| s.approx_tokens).sum();
    if total == 0 {
        return String::new();
    }

    let dominant = analysis
        .context_map
        .iter()
        .max_by_key(|source| source.approx_tokens);

    let Some(dominant) = dominant else {
        return String::new();
    };

    let share = dominant.approx_tokens as f64 / total as f64;
    let glyph = gauge_glyph(share);
    let pct = (share * 100.0).round() as usize;
    let tag = kind_tag(&dominant.source_kind);

    let warning_count = analysis.warnings.len();
    let severity = worst_severity(analysis);

    let mut line = format!("{glyph} {pct}% [{tag}]");
    if warning_count > 0 {
        line.push_str(&format!(" · {warning_count}⚠"));
    }

    if use_color {
        line.style(severity_style(severity.as_ref())).to_string()
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ContextSourceKind, ContextSourceSummary, ContextWarning};

    fn analysis(sources: Vec<(usize, ContextSourceKind)>, warnings: Vec<WarningSeverity>) -> SessionAnalysis {
        SessionAnalysis {
            session_id: "test".to_string(),
            instruction_files_loaded: 0,
            files_read: 0,
            file_searches: 0,
            file_path_lists: 0,
            bash_commands: 0,
            files_edited: 0,
            files_written: 0,
            subagents: 0,
            approx_context_tokens: sources.iter().map(|(t, _)| t).sum(),
            context_map: sources
                .into_iter()
                .map(|(t, kind)| ContextSourceSummary {
                    source_kind: kind,
                    source_label: "label".to_string(),
                    occurrences: 1,
                    approx_tokens: t,
                    trigger_reasons: Vec::new(),
                })
                .collect(),
            events: Vec::new(),
            warnings: warnings
                .into_iter()
                .map(|severity| ContextWarning {
                    severity,
                    title: "warning".to_string(),
                    detail: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn empty_session_produces_no_line() {
        let a = analysis(vec![], vec![]);
        assert_eq!(status_line(&a, false), "");
    }

    #[test]
    fn zero_token_sources_produce_no_line() {
        let a = analysis(vec![(0, ContextSourceKind::ShellOutput)], vec![]);
        assert_eq!(status_line(&a, false), "");
    }

    #[test]
    fn clean_session_shows_glyph_percent_and_kind_tag() {
        let a = analysis(
            vec![(600, ContextSourceKind::ShellOutput), (400, ContextSourceKind::FileRead)],
            vec![],
        );
        let line = status_line(&a, false);
        assert_eq!(line, "◑ 60% [shell]");
    }

    #[test]
    fn warnings_append_a_count_suffix() {
        let a = analysis(
            vec![(1000, ContextSourceKind::ShellOutput)],
            vec![WarningSeverity::Medium, WarningSeverity::Low],
        );
        let line = status_line(&a, false);
        assert!(line.ends_with(" · 2⚠"), "got {line:?}");
    }

    #[test]
    fn no_warnings_have_no_suffix() {
        let a = analysis(vec![(1000, ContextSourceKind::ShellOutput)], vec![]);
        assert!(!status_line(&a, false).contains('⚠'));
    }

    #[test]
    fn gauge_glyph_spans_full_range() {
        assert_eq!(gauge_glyph(0.0), '░');
        assert_eq!(gauge_glyph(0.25), '◔');
        assert_eq!(gauge_glyph(0.5), '◑');
        assert_eq!(gauge_glyph(0.75), '◕');
        assert_eq!(gauge_glyph(1.0), '●');
    }

    #[test]
    fn colour_disabled_emits_no_ansi() {
        let a = analysis(
            vec![(1000, ContextSourceKind::ShellOutput)],
            vec![WarningSeverity::High],
        );
        assert!(!status_line(&a, false).contains('\u{1b}'));
    }

    #[test]
    fn colour_enabled_emits_ansi() {
        let a = analysis(vec![(1000, ContextSourceKind::ShellOutput)], vec![]);
        assert!(status_line(&a, true).contains('\u{1b}'));
    }

    #[test]
    fn high_severity_warning_wins_over_medium() {
        let a = analysis(
            vec![(1000, ContextSourceKind::ShellOutput)],
            vec![WarningSeverity::Medium, WarningSeverity::High],
        );
        assert_eq!(worst_severity(&a), Some(WarningSeverity::High));
    }
}
