use crate::model::{SessionAnalysis, WarningSeverity};

#[derive(Debug, Clone)]
pub struct TrendPoint {
    pub session_id: String,
    pub approx_context_tokens: usize,
    pub high_warnings: usize,
    pub medium_warnings: usize,
    pub low_warnings: usize,
}

pub fn build_trend_points(analyses: &[SessionAnalysis]) -> Vec<TrendPoint> {
    analyses
        .iter()
        .map(|analysis| {
            let mut point = TrendPoint {
                session_id: analysis.session_id.clone(),
                approx_context_tokens: analysis.approx_context_tokens,
                high_warnings: 0,
                medium_warnings: 0,
                low_warnings: 0,
            };
            for warning in &analysis.warnings {
                match warning.severity {
                    WarningSeverity::High => point.high_warnings += 1,
                    WarningSeverity::Medium => point.medium_warnings += 1,
                    WarningSeverity::Low => point.low_warnings += 1,
                }
            }
            point
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContextWarning;

    fn analysis(id: &str, tokens: usize, severities: &[WarningSeverity]) -> SessionAnalysis {
        SessionAnalysis {
            session_id: id.to_string(),
            instruction_files_loaded: 0,
            files_read: 0,
            file_searches: 0,
            file_path_lists: 0,
            bash_commands: 0,
            files_edited: 0,
            files_written: 0,
            subagents: 0,
            approx_context_tokens: tokens,
            context_map: Vec::new(),
            events: Vec::new(),
            warnings: severities
                .iter()
                .map(|s| ContextWarning {
                    severity: s.clone(),
                    title: "t".into(),
                    detail: "d".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn maps_one_point_per_session_in_given_order() {
        let sessions = vec![
            analysis("s1", 100, &[WarningSeverity::High]),
            analysis(
                "s2",
                200,
                &[WarningSeverity::Medium, WarningSeverity::Medium],
            ),
        ];
        let points = build_trend_points(&sessions);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].session_id, "s1");
        assert_eq!(points[0].approx_context_tokens, 100);
        assert_eq!(points[0].high_warnings, 1);
        assert_eq!(points[1].session_id, "s2");
        assert_eq!(points[1].medium_warnings, 2);
    }

    #[test]
    fn counts_each_severity_independently() {
        let sessions = vec![analysis(
            "s1",
            50,
            &[
                WarningSeverity::High,
                WarningSeverity::Low,
                WarningSeverity::Low,
            ],
        )];
        let points = build_trend_points(&sessions);
        assert_eq!(points[0].high_warnings, 1);
        assert_eq!(points[0].medium_warnings, 0);
        assert_eq!(points[0].low_warnings, 2);
    }

    #[test]
    fn empty_input_produces_empty_output() {
        assert!(build_trend_points(&[]).is_empty());
    }
}
