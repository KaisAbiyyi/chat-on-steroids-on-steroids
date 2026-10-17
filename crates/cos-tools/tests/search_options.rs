use cos_core::config::Root;
use cos_tools::{
    files::Files,
    search::{self, Options},
};
#[test]
fn case_regex_globs_and_single_file() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.rs"), "Alpha\nalpha42\nBeta\n").unwrap();
    std::fs::write(d.path().join("b.txt"), "alpha99\n").unwrap();
    let roots = [Root {
        name: "p".into(),
        path: std::fs::canonicalize(d.path())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }];
    let f = Files::default();
    let opts = Options {
        content: true,
        case_sensitive: true,
        regex: true,
        glob: Some("**/*.rs"),
        ..Options::new("alpha[0-9]+")
    };
    let result = search::find_options(&f, &roots, "/p", &opts).unwrap();
    assert_eq!(result["matches"].as_array().unwrap().len(), 1);
    assert_eq!(result["matches"][0]["line"], 2);
    let result = search::find_options(
        &f,
        &roots,
        "/p/a.rs",
        &Options {
            content: true,
            case_sensitive: true,
            ..Options::new("Alpha")
        },
    )
    .unwrap();
    assert_eq!(result["matches"].as_array().unwrap().len(), 1);
}
