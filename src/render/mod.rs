mod graph_data;
mod html;
mod trend_data;

use crate::analyse;
use crate::config::CcmapConfig;
use crate::storage::Storage;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn write_graph(
    storage: &Storage,
    config: &CcmapConfig,
    session_path: &Path,
) -> Result<PathBuf> {
    let target_analysis = analyse::analyse_file(session_path, config)?;

    let history_paths = storage.session_files_ordered_by_time()?;
    let history_analyses: Vec<_> = history_paths
        .iter()
        .filter_map(|path| analyse::analyse_file(path, config).ok())
        .collect();

    if history_analyses.len() != history_paths.len() {
        eprintln!(
            "note: skipped {} unparseable session file(s) in trend history",
            history_paths.len() - history_analyses.len()
        );
    }

    let graph = graph_data::build_graph_data(&target_analysis);
    let trend = trend_data::build_trend_points(&history_analyses);
    let document = html::render_html(&target_analysis.session_id, &graph, &trend);

    storage.create_dirs()?;
    let output_path = storage
        .reports_dir
        .join(format!("{}-graph.html", target_analysis.session_id));
    std::fs::write(&output_path, document)?;

    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CcmapConfig;
    use crate::project::Project;
    use crate::storage::Storage;
    use std::fs;

    fn temp_project(name: &str) -> (Project, Storage) {
        let root =
            std::env::temp_dir().join(format!("ccmap-render-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            root: root.clone(),
            name: "test".into(),
            id: "test-id".into(),
        };
        let storage = Storage::for_project(&project);
        storage.create_dirs().unwrap();
        (project, storage)
    }

    #[test]
    fn write_graph_produces_an_html_file_named_after_the_session() {
        let (_project, storage) = temp_project("write-graph");
        let session_path = storage.sessions_dir.join("demo.jsonl");
        fs::write(
            &session_path,
            concat!(
                "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"demo\",\"cwd\":\"/repo\"}\n",
                "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"demo\",\"tool_name\":\"Read\",",
                "\"tool_input\":{\"file_path\":\"/repo/src/main.rs\"},",
                "\"tool_response\":{\"content\":\"fn main() {}\"}}\n",
            ),
        )
        .unwrap();

        let config = CcmapConfig::default();
        let output = write_graph(&storage, &config, &session_path).unwrap();

        assert!(output.exists());
        assert_eq!(
            output.file_name().unwrap().to_string_lossy(),
            "demo-graph.html"
        );
        let contents = fs::read_to_string(&output).unwrap();
        assert!(contents.contains("<!doctype html>"));
        assert!(contents.contains("main.rs"));

        let _ = fs::remove_dir_all(storage.base_dir.parent().unwrap());
    }

    #[test]
    fn write_graph_succeeds_when_history_contains_an_unparseable_session_file() {
        // Regression test for the silent-skip diagnostic: one valid session
        // file plus one corrupt/unparseable one in the project's history
        // must not cause write_graph to fail — the corrupt file should be
        // skipped gracefully (with a stderr note, not asserted here) while
        // the function still returns Ok.
        let (_project, storage) = temp_project("write-graph-corrupt-history");

        let valid_path = storage.sessions_dir.join("valid.jsonl");
        fs::write(
            &valid_path,
            concat!(
                "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"valid\",\"cwd\":\"/repo\"}\n",
                "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"valid\",\"tool_name\":\"Read\",",
                "\"tool_input\":{\"file_path\":\"/repo/src/lib.rs\"},",
                "\"tool_response\":{\"content\":\"fn lib() {}\"}}\n",
            ),
        )
        .unwrap();

        let corrupt_path = storage.sessions_dir.join("corrupt.jsonl");
        fs::write(&corrupt_path, "{ this is not valid jsonl at all\n").unwrap();

        let config = CcmapConfig::default();
        let output = write_graph(&storage, &config, &valid_path);

        assert!(
            output.is_ok(),
            "write_graph should skip unparseable history files without failing, got: {output:?}"
        );
        assert!(output.unwrap().exists());

        let _ = fs::remove_dir_all(storage.base_dir.parent().unwrap());
    }
}
