use cos_tools::patch::{self, ParseMode, StreamingParser};
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/patch-streaming.json"
    ))
    .unwrap()
}
#[test]
fn native_stream_steps_and_finish_match_actual_upstream() {
    for case in fixture()["streams"].as_array().unwrap() {
        let mut parser = StreamingParser::default();
        for step in case["steps"].as_array().unwrap() {
            let actual = parser.push_delta(step["delta"].as_str().unwrap());
            match actual {
                Ok(hunks) => assert_eq!(
                    serde_json::to_value(hunks).unwrap(),
                    step["expected"],
                    "{step}"
                ),
                Err(error) => assert_eq!(Some(error.as_str()), step["error"].as_str(), "{step}"),
            }
            assert_eq!(
                serde_json::to_value(parser.environment_id()).unwrap(),
                step["environmentId"]
            );
        }
        let finish = &case["finish"];
        match parser.finish() {
            Ok(hunks) => assert_eq!(
                serde_json::to_value(hunks).unwrap(),
                finish["expected"],
                "{finish}"
            ),
            Err(error) => assert_eq!(Some(error.as_str()), finish["error"].as_str(), "{finish}"),
        }
        assert_eq!(
            serde_json::to_value(parser.environment_id()).unwrap(),
            finish["environmentId"]
        );
    }
}
#[test]
fn native_strict_and_lenient_parsing_match_actual_upstream() {
    for case in fixture()["parsing"].as_array().unwrap() {
        let mode = if case["mode"] == "strict" {
            ParseMode::Strict
        } else {
            ParseMode::Lenient
        };
        match patch::parse_typed_mode(case["patch"].as_str().unwrap(), mode) {
            Ok(parsed) => assert_eq!(
                serde_json::to_value(parsed).unwrap(),
                case["expected"],
                "{case}"
            ),
            Err(error) => assert_eq!(Some(error.as_str()), case["error"].as_str(), "{case}"),
        }
    }
}
#[test]
fn streaming_input_is_admitted_before_an_unterminated_buffer_can_exceed_its_limit() {
    let mut parser = StreamingParser::default();
    parser.push_delta(&"x".repeat(4 * 1024 * 1024)).unwrap();
    assert!(
        parser.push_delta("x").is_err(),
        "The streaming parser admitted more than its 4 MiB UTF-16 limit"
    );
}
