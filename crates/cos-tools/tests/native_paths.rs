use cos_core::config::Root;
use cos_tools::files::Files;
#[test]
fn ordinary_windows_path_spelling_is_authorized() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a"), "public fixture").unwrap();
    let path = std::fs::canonicalize(d.path())
        .unwrap()
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_owned();
    let roots = [Root {
        name: "p".into(),
        path,
        remote: None,
    }];
    let result = Files::default().read(&roots, "/p/a", 1, 10, 1024).unwrap();
    assert_eq!(result["text"], "public fixture");
}
