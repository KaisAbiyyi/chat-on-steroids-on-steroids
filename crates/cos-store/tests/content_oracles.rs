use cos_store::{
    content::{self, TextError},
    dil, markdown,
};
use serde_json::{Value, json};

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-content.json"
    ))
    .unwrap()
}
fn expect(result: Result<Value, TextError>, expected: &Value, input: &Value) {
    if expected["needsLosslessString"] == true {
        assert!(matches!(result, Err(TextError::UnpairedUtf16)), "{input}");
    } else if expected["error"].is_object() {
        match result {
            Err(TextError::JavaScript { name, message }) => assert_eq!(
                json!({"name":name,"message":message}),
                expected["error"],
                "{input}"
            ),
            other => panic!("Expected source error for {input}: {other:?}"),
        }
    } else {
        assert_eq!(result.unwrap(), expected["value"], "{input}");
    }
}
fn compare(names: &[&str]) {
    for case in corpus()["pure"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !names.contains(&name) {
            continue;
        }
        let args = case["args"].as_array().unwrap();
        let text = args[0].as_str().unwrap_or("");
        let result = match name {
            "hasProviderDirective" => Ok(json!(content::has_provider_directive(text))),
            "hasContentReference" => Ok(json!(content::has_content_reference(text))),
            "withoutProviderDirectives" => Ok(json!(content::without_provider_directives(text))),
            "plainTextOfHtml" => content::plain_text_of_html(text).map(|v| json!(v)),
            "providerEntitiesAsMarkdown" => Ok(json!(content::provider_entities_as_markdown(text))),
            "resolvedCapture" => content::resolved_capture(args.first()).map(|v| json!(v)),
            "modelFacingText" => content::model_facing_text(text, args.get(1)).map(|v| json!(v)),
            _ => unreachable!(),
        };
        expect(result, &case["expected"], case);
    }
}
#[test]
fn directive_grammar_retains_source_ascii_and_javascript_line_boundaries() {
    compare(&[
        "hasProviderDirective",
        "hasContentReference",
        "withoutProviderDirectives",
    ]);
}
#[test]
fn captured_html_decoding_and_private_provider_entities_match_source() {
    compare(&["plainTextOfHtml", "providerEntitiesAsMarkdown"]);
}
#[test]
fn resolved_capture_and_model_text_preserve_source_fallback_and_errors() {
    compare(&["resolvedCapture", "modelFacingText"]);
}
#[test]
fn dil_table_completion_preserves_unmatched_text_and_capture_admission() {
    for case in corpus()["tableCases"].as_array().unwrap() {
        expect(
            dil::completed_dil_text(case["text"].as_str().unwrap(), case.get("capture"))
                .map(|v| json!(v)),
            &case["expected"],
            case,
        );
    }
}
#[test]
fn transcript_deduplication_and_final_selection_match_the_shared_source() {
    for case in corpus()["transcripts"].as_array().unwrap() {
        assert_eq!(
            json!(markdown::transcript_entries(
                case["events"].as_array().unwrap()
            )),
            case["expected"]
        );
    }
}
#[test]
fn answer_completion_anchors_and_formatting_match_the_shared_source() {
    let fixture = corpus();
    assert_eq!(
        json!(markdown::completed_turn_ids(
            fixture["lifecycle"].as_array().unwrap()
        )),
        fixture["completed"]
    );
    let anchors = &fixture["anchors"];
    let events = anchors["events"].as_array().unwrap();
    assert_eq!(
        json!(markdown::completed_turn_ids(events)),
        anchors["completed"]
    );
    assert_eq!(json!(markdown::answer_anchors(events)), anchors["expected"]);
    for answer in anchors["answers"].as_array().unwrap() {
        assert_eq!(
            json!(markdown::turn_answer_entries(
                events,
                answer["turnId"].as_str().unwrap()
            )),
            answer["expected"]
        );
    }
    for case in fixture["formatting"].as_array().unwrap() {
        assert_eq!(
            markdown::session_markdown(
                case["title"].as_str().unwrap(),
                case["entries"].as_array().unwrap(),
                case["labels"]["user"].as_str().unwrap_or("You"),
                case["labels"]["assistant"].as_str().unwrap_or("ChatGPT")
            ),
            case["expected"]
        );
    }
}
#[test]
fn markdown_filenames_keep_source_nfc_and_avoid_split_surrogates() {
    for case in corpus()["filenames"].as_array().unwrap() {
        assert_eq!(
            markdown::markdown_file_name(
                case["title"].as_str().unwrap(),
                case["suffix"].as_str().unwrap()
            ),
            case["expected"]
        );
    }
}
