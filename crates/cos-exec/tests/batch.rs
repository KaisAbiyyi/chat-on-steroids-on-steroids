use cos_exec::batch;
use serde_json::Value;
#[test]
fn upstream_shell_batch_programs() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/command-batch.json"
    ))
    .unwrap();
    for c in cases.as_array().unwrap() {
        let commands = c["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            batch::compose(
                &commands,
                c["shell"].as_str().unwrap(),
                c["marker"].as_str().unwrap()
            ),
            c["expected"]
        );
    }
}
