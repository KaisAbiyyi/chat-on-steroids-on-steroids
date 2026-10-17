use cos_core::config::Root;
use cos_tools::files::Files;
#[test]
fn huge_file_small_read_is_bounded_and_reports_unknown_total() {
    let d = tempfile::tempdir().unwrap();
    let mut file = std::fs::File::create(d.path().join("large")).unwrap();
    use std::io::Write;
    for _ in 0..60000 {
        file.write_all(&[b'x'; 100]).unwrap();
        file.write_all(b"\n").unwrap();
    }
    let roots = [Root {
        name: "p".into(),
        path: std::fs::canonicalize(d.path())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }];
    let r = Files::default()
        .read(&roots, "/p/large", 1, 2, 1000)
        .unwrap();
    assert_eq!(r["lastLine"], 2);
    assert_eq!(r["text"].as_str().unwrap().len(), 201);
    assert_eq!(r["totalLines"], serde_json::Value::Null);
    assert_eq!(r["more"], true);
}
