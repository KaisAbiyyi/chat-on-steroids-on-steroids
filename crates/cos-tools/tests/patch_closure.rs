use cos_tools::patch::{self, Chunk};
use serde_json::Value;

#[test]
fn source_patch_update_modes_seek_and_diagnostics_match() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/patch-updates.json"
    ))
    .unwrap();
    for case in fixture["updates"].as_array().unwrap() {
        let chunks: Vec<Chunk> = serde_json::from_value(case["chunks"].clone()).unwrap();
        let mode = serde_json::from_value(case["mode"].clone()).unwrap();
        let result = patch::update_with_mode(
            case["contents"].as_str().unwrap(),
            "public.txt",
            &chunks,
            mode,
        );
        match result {
            Ok(text) => assert_eq!(Some(text.as_str()), case["expected"].as_str(), "{case}"),
            Err(error) => {
                assert_eq!(
                    Some(error.message.as_str()),
                    case["error"].as_str(),
                    "{case}"
                );
                assert_eq!(
                    serde_json::to_value(error.source_context).unwrap(),
                    case["sourceContext"],
                    "{case}"
                );
            }
        }
    }
    for case in fixture["seeks"].as_array().unwrap() {
        let lines: Vec<String> = serde_json::from_value(case["lines"].clone()).unwrap();
        let pattern: Vec<String> = serde_json::from_value(case["pattern"].clone()).unwrap();
        let actual = patch::seek_with_mode(
            &lines,
            &pattern,
            case["start"].as_u64().unwrap() as usize,
            case["eof"].as_bool().unwrap(),
            serde_json::from_value(case["mode"].clone()).unwrap(),
        );
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            case["expected"],
            "{case}"
        );
    }
}
