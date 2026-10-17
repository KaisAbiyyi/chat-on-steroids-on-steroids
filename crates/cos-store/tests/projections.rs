use cos_store::{
    provider_usage::{ClaudeFile, CodexFile, ProviderUsage},
    views,
};
use serde_json::{Value, json};
#[test]
fn provider_log_folding_matches_upstream_cumulative_resets_and_dedup() {
    let case: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/provider-cases.json"
    ))
    .unwrap();
    let mut codex = CodexFile::default();
    for line in case["codexLines"].as_array().unwrap() {
        codex.fold(line.as_str().unwrap());
    }
    assert_eq!(codex.snapshot(), case["codex"]);
    let mut claude = ClaudeFile::default();
    for line in case["claudeLines"].as_array().unwrap() {
        claude.fold(line.as_str().unwrap());
    }
    assert_eq!(
        ClaudeFile::totals(&[claude.clone(), claude]),
        case["claude"]
    );
}
#[test]
fn provider_reads_only_explicit_test_roots_and_respects_opt_out() {
    let case: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/provider-cases.json"
    ))
    .unwrap();
    let codex = tempfile::tempdir().unwrap();
    let claude = tempfile::tempdir().unwrap();
    let data = codex.path().join("sessions/2026/10/16");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(
        data.join("public.jsonl"),
        case["codexLines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| format!("{}\n", l.as_str().unwrap()))
            .collect::<String>(),
    )
    .unwrap();
    let reply = ProviderUsage::read(true, 1792116000000, codex.path(), claude.path()).unwrap();
    assert_eq!(reply["codex"]["days"][6]["input"], 155);
    assert_eq!(reply["claude"], Value::Null);
    assert_eq!(
        ProviderUsage::read(false, 1, codex.path(), claude.path()).unwrap(),
        json!({"enabled":false,"readAt":1,"codex":null,"claude":null})
    );
}
#[test]
fn accent_and_unicode_search_folding_match_upstream() {
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/upstream/search-cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        assert_eq!(
            views::fold_case(case["text"].as_str().unwrap()),
            case["expected"].as_str().unwrap()
        );
    }
}
#[test]
fn provider_scan_budget_and_stale_filter_are_visible() {
    let case: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/provider-cases.json"
    ))
    .unwrap();
    let codex = tempfile::tempdir().unwrap();
    let claude = tempfile::tempdir().unwrap();
    let folder = codex.path().join("sessions/2026/10/16");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("public.jsonl");
    std::fs::write(
        &file,
        format!("{}\n", case["codexLines"][0].as_str().unwrap()),
    )
    .unwrap();
    let reply =
        ProviderUsage::read_with_budget(true, 1792116000000, codex.path(), claude.path(), 1)
            .unwrap();
    assert_eq!(reply["limited"], true);
    assert_eq!(reply["scannedBytes"], 1);
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(old))
        .unwrap();
    let reply = ProviderUsage::read(true, 1792116000000, codex.path(), claude.path()).unwrap();
    assert_eq!(reply["codex"]["days"][6]["input"], 0);
}
