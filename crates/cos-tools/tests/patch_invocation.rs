use cos_tools::patch::invocation;
use serde_json::Value;
#[test]
fn invocation_classification_and_errors_match_actual_upstream_grammar() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/patch-invocation.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let argv = serde_json::from_value::<Vec<String>>(case["argv"].clone()).unwrap();
        let cwd = case["cwd"].as_str().unwrap();
        let result = if case["mode"] == "exec" {
            invocation::for_exec(&argv, cwd)
        } else {
            invocation::maybe_parse(&argv, cwd)
        };
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            case["expected"],
            "{case}"
        );
    }
}
#[test]
fn invocation_resource_admission_precedes_bash_tree_allocation() {
    let argv = vec!["bash".into(), "-c".into(), "x".repeat(4 * 1024 * 1024 + 1)];
    assert!(matches!(
        invocation::for_exec(&argv, "/public"),
        invocation::Invocation::CorrectnessError { .. }
    ));
}
