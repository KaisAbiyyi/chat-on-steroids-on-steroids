use cos_core::config::Root;
use cos_tools::{
    files::Files,
    search::{self, Options},
};
use std::{fs, io};

fn setup() -> (tempfile::TempDir, Vec<Root>) {
    let temp = tempfile::tempdir().unwrap();
    let roots = vec![Root {
        name: "p".into(),
        path: fs::canonicalize(temp.path())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }];
    (temp, roots)
}

#[test]
fn malformed_regex_is_rejected_even_for_empty_directory_or_skipped_file() {
    let (temp, roots) = setup();
    let opts = Options {
        content: true,
        regex: true,
        ..Options::new("[unterminated")
    };
    let error =
        search::find_checked(&Files::default(), &roots, "/p", &opts, &|| Ok(())).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    fs::write(temp.path().join("big.txt"), vec![0; 2 * 1024 * 1024 + 1]).unwrap();
    let error = search::find_checked(&Files::default(), &roots, "/p/big.txt", &opts, &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn unsupported_js_backreference_and_lookaround_fail_explicitly() {
    let (_temp, roots) = setup();
    for pattern in [r"(a)\1", "foo(?=bar)", "(?<!no)yes"] {
        let opts = Options {
            content: true,
            regex: true,
            ..Options::new(pattern)
        };
        let error =
            search::find_checked(&Files::default(), &roots, "/p", &opts, &|| Ok(())).unwrap_err();
        assert_eq!(
            error.kind(),
            io::ErrorKind::Unsupported,
            "{pattern}: {error}"
        );
    }
}

#[test]
fn regex_result_limit_preserves_bounded_hits_and_truncation() {
    let (temp, roots) = setup();
    fs::write(temp.path().join("hits.txt"), "ID1\nID2\nID3\n").unwrap();
    let result = search::find_checked(
        &Files::default(),
        &roots,
        "/p/hits.txt",
        &Options {
            content: true,
            regex: true,
            limit: 2,
            ..Options::new(r"ID[0-9]+")
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result["matches"].as_array().unwrap().len(), 2);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["stopped_because"], "limit");
}

#[test]
fn search_query_admission_is_bounded_before_directory_io() {
    let (_temp, roots) = setup();
    let large = "q".repeat(64 * 1024 + 1);
    let error = search::find_checked(
        &Files::default(),
        &roots,
        "/p",
        &Options::new(&large),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}
