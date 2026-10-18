use cos_core::config::Root;
use cos_tools::{
    files::Files,
    search::{self, IncludeMatcher, Options},
};
use serde_json::{Value, json};
use std::fs;
fn roots(path: &std::path::Path) -> [Root; 1] {
    [Root {
        name: "p".into(),
        path: fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }]
}
#[test]
fn include_patterns_match_source_utf16_and_case_rules() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/search-globs.json"
    ))
    .unwrap();
    assert_eq!(json!(search::DEFAULT_EXCLUDES), corpus["defaultExcludes"]);
    let platform = if cfg!(windows) { "win32" } else { "linux" };
    for case in corpus["globs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["platform"] == platform)
    {
        let matcher = IncludeMatcher::new(
            case["pattern"].as_str().unwrap(),
            case["sensitive"].as_bool().unwrap(),
        )
        .unwrap();
        assert_eq!(
            matcher.matches(case["file"].as_str().unwrap()),
            case["expected"].as_bool().unwrap(),
            "{case}"
        );
    }
    for case in corpus["excludes"].as_array().unwrap() {
        let patterns: Vec<String> = serde_json::from_value(case["patterns"].clone()).unwrap();
        assert_eq!(
            cos_tools::files::is_excluded_folder(case["name"].as_str().unwrap(), &patterns),
            case["expected"].as_bool().unwrap(),
            "{case}"
        );
    }
}
#[test]
fn name_queries_match_the_filename_instead_of_the_parent_path() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("public-parent")).unwrap();
    fs::write(temp.path().join("public-parent/needle.rs"), "public").unwrap();
    let result = search::find_options(
        &Files::default(),
        &roots(temp.path()),
        "/p",
        &Options::new("public-parent"),
    )
    .unwrap();
    assert_eq!(result["matches"], json!([]), "{result}");
}
#[test]
fn native_include_matches_nested_basenames_and_utf16_units() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("nested")).unwrap();
    fs::write(temp.path().join("nested/needle.rs"), "public").unwrap();
    fs::write(temp.path().join("file😀.txt"), "public").unwrap();
    let root = roots(temp.path());
    let result = search::find_options(
        &Files::default(),
        &root,
        "/p",
        &Options {
            glob: Some("*.RS"),
            ..Options::new("needle")
        },
    )
    .unwrap();
    assert_eq!(
        result["matches"],
        json!([{"path":"/p/nested/needle.rs"}]),
        "{result}"
    );
    let result = search::find_options(
        &Files::default(),
        &root,
        "/p",
        &Options {
            glob: Some("file?.txt"),
            ..Options::new("file")
        },
    )
    .unwrap();
    assert_eq!(
        result["matches"],
        json!([]),
        "JS's non-Unicode ? matches one UTF-16 unit: {result}"
    );
}
#[test]
fn explicit_unicode_folder_excludes_match_source_lowercasing() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("Äbuild")).unwrap();
    fs::write(temp.path().join("Äbuild/needle.txt"), "public").unwrap();
    let exclude = vec!["äbuild".into()];
    let result = search::find_options(
        &Files::default(),
        &roots(temp.path()),
        "/p",
        &Options {
            exclude: Some(&exclude),
            ..Options::new("needle")
        },
    )
    .unwrap();
    assert_eq!(result["matches"], json!([]), "{result}");
}
