use cos_store::{CreateOptions, SessionStore};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-content.json"
    ))
    .unwrap()
}
fn compare_exports(names: &[&str]) {
    for case in corpus()["exports"].as_array().unwrap() {
        if !names.contains(&case["name"].as_str().unwrap()) {
            continue;
        }
        let directory = tempfile::tempdir().unwrap();
        let id = "session-export-public";
        let store = SessionStore::open(directory.path()).unwrap();
        store
            .create(CreateOptions {
                id: Some(id.into()),
                title: Some(case["title"].as_str().unwrap().into()),
                ..Default::default()
            })
            .unwrap();
        for (name, text) in case["overflow"].as_object().unwrap() {
            assert_eq!(
                store
                    .write_asset(id, text.as_str().unwrap().as_bytes(), "text/plain")
                    .unwrap()["id"],
                name.as_str()
            );
        }
        drop(store);
        let journal = directory
            .path()
            .join("sessions")
            .join(id)
            .join("events.jsonl");
        let rows = case["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| format!("{event}\n"))
            .collect::<String>();
        std::fs::write(journal, rows).unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        for expected in case["outcomes"].as_array().unwrap() {
            let result = if expected["scope"] == "answer" {
                store.export_answer_markdown(id, "turn-public")
            } else {
                store.export_markdown(id)
            };
            if let Some(error) = expected["error"].as_str() {
                assert_eq!(result.unwrap_err().to_string(), error);
            } else {
                assert_eq!(
                    result.unwrap(),
                    expected["text"],
                    "{} {}",
                    case["name"],
                    expected["scope"]
                );
            }
        }
    }
}
#[test]
fn markdown_link_cards_match_the_actual_source_export_function() {
    compare_exports(&["entities"]);
}
#[test]
fn drawn_table_rows_and_dead_button_labels_match_the_actual_source_export_function() {
    compare_exports(&["drawn-table"]);
}
#[test]
fn markdown_hydrates_only_truncated_stored_text() {
    compare_exports(&["overflow"]);
}
#[test]
fn session_and_answer_export_keep_the_original_completion_boundary() {
    compare_exports(&["incomplete"]);
}
