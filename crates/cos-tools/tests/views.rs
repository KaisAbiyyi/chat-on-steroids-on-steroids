use cos_core::config::Root;
use cos_tools::{files::Files, image, search};
#[test]
fn search_exclusions_order_and_image_decode() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("node_modules")).unwrap();
    std::fs::write(d.path().join("node_modules/hidden"), "public needle").unwrap();
    std::fs::write(d.path().join("b.txt"), "b\nneedle\n").unwrap();
    std::fs::write(d.path().join("a.txt"), "needle\n").unwrap();
    let roots = [Root {
        name: "p".into(),
        path: std::fs::canonicalize(d.path())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }];
    let r = search::find_options(
        &Files::default(),
        &roots,
        "/p",
        &search::Options {
            content: true,
            limit: 100,
            ..search::Options::new("needle")
        },
    )
    .unwrap();
    assert_eq!(r["matches"].as_array().unwrap().len(), 2);
    assert_eq!(r["matches"][0]["path"], "/p/a.txt");
    assert_eq!(r["matches"][1]["line"], 2);
    let mut png = std::io::Cursor::new(vec![]);
    ::image::DynamicImage::new_rgba8(2, 2)
        .write_to(&mut png, ::image::ImageFormat::Png)
        .unwrap();
    assert_eq!(image::validate(png.get_ref()).unwrap(), "image/png");
    assert!(image::validate(b"\x89PNG\r\n\x1a\ninvalid").is_err());
}
