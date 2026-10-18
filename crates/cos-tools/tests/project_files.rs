use base64::{Engine, engine::general_purpose::STANDARD};
use cos_core::{config::Root, persistence::DurableStore};
use cos_tools::{
    project::Projects,
    project_files::{Edit, ProjectFiles},
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
struct Fixture {
    _profile: tempfile::TempDir,
    root: tempfile::TempDir,
    folder: PathBuf,
    roots: [Root; 1],
    projects: Projects,
    id: String,
    files: ProjectFiles,
}
impl Fixture {
    fn new() -> Self {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("project");
        fs::create_dir(&folder).unwrap();
        let roots = [Root {
            name: "r".into(),
            path: fs::canonicalize(root.path())
                .unwrap()
                .to_string_lossy()
                .into(),
            remote: None,
        }];
        let projects = Projects::open(DurableStore::open(profile.path()).unwrap());
        let p = projects.add(&roots, "/r/project").unwrap();
        Self {
            _profile: profile,
            root,
            folder,
            roots,
            projects,
            id: p["id"].as_str().unwrap().into(),
            files: ProjectFiles::default(),
        }
    }
    fn preview(&self, path: &str) -> Value {
        self.files
            .preview(&self.projects, &self.roots, &self.id, path)
            .unwrap()
    }
    fn save(&self, path: &str, text: &str, preview: &Value) -> std::io::Result<Value> {
        self.files.save(
            &self.projects,
            &self.roots,
            &self.id,
            path,
            &Edit {
                text,
                modified_at: preview["modifiedAt"].as_str().unwrap(),
                bytes: preview["bytes"].as_u64().unwrap(),
                revision: preview["revision"].as_str().unwrap(),
            },
        )
    }
}
#[test]
fn explorer_lists_only_selected_project_children_and_hides_repository_internals() {
    let f = Fixture::new();
    fs::create_dir(f.folder.join("nested")).unwrap();
    fs::create_dir(f.folder.join(".git")).unwrap();
    fs::write(f.folder.join(".DS_Store"), "private metadata").unwrap();
    fs::write(f.folder.join("z.txt"), "public").unwrap();
    let listing = f.files.list(&f.projects, &f.roots, &f.id, "").unwrap();
    assert_eq!(
        listing["entries"],
        json!([{"name":"nested","path":"nested","kind":"directory","bytes":null},{"name":"z.txt","path":"z.txt","kind":"file","bytes":null}])
    );
    assert_eq!(listing["projectName"], "project");
    for path in [
        "../outside",
        "nested/..",
        "/r",
        "nested//a",
        "nested/./a",
        "C:/outside",
    ] {
        assert!(
            f.files.list(&f.projects, &f.roots, &f.id, path).is_err(),
            "{path}"
        );
    }
}
#[test]
fn source_directory_order_and_truncation_match_native_project_listing() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/project-files.json"
    ))
    .unwrap();
    for case in fixture["directories"].as_array().unwrap() {
        let f = Fixture::new();
        for row in case["names"].as_array().unwrap() {
            let path = f.folder.join(row[0].as_str().unwrap());
            if row[1] == "directory" {
                fs::create_dir(path).unwrap();
            } else {
                fs::write(path, "public").unwrap();
            }
        }
        let listing = f.files.list(&f.projects, &f.roots, &f.id, "").unwrap();
        assert_eq!(
            json!({"entries":listing["entries"],"truncated":listing["truncated"]}),
            case["expected"]
        );
    }
}
#[test]
fn preview_and_atomic_save_preserve_exact_bom_and_crlf_text() {
    let f = Fixture::new();
    fs::write(f.folder.join("a.txt"), b"\xef\xbb\xbfpublic\r\nlast").unwrap();
    let preview = f.preview("a.txt");
    assert_eq!(preview["text"], "\u{feff}public\r\nlast");
    assert_eq!(preview["binary"], false);
    assert_eq!(preview["truncated"], false);
    let saved = f.save("a.txt", "\u{feff}edited\r\nlast", &preview).unwrap();
    assert_eq!(saved["preview"]["text"], "\u{feff}edited\r\nlast");
    assert_eq!(
        fs::read(f.folder.join("a.txt")).unwrap(),
        b"\xef\xbb\xbfedited\r\nlast"
    );
    assert!(f.save("a.txt", "stale edit", &preview).is_err());
    assert_eq!(fs::read_dir(&f.folder).unwrap().count(), 1);
}
#[test]
fn file_modified_at_uses_the_source_node_stats_millisecond_rounding() {
    let f = Fixture::new();
    fs::write(f.folder.join("a.txt"), "public").unwrap();
    for (nanos, expected) in [
        (123_400_000, "2023-11-14T22:13:20.123Z"),
        (123_600_000, "2023-11-14T22:13:20.124Z"),
        (999_600_000, "2023-11-14T22:13:21.000Z"),
    ] {
        fs::OpenOptions::new()
            .write(true)
            .open(f.folder.join("a.txt"))
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::new(1_700_000_000, nanos),
            ))
            .unwrap();
        assert_eq!(f.preview("a.txt")["modifiedAt"], expected);
    }
}
#[test]
fn noneditable_and_large_preview_use_source_metadata_time_and_content_detection() {
    let f = Fixture::new();
    for (name, bytes, binary) in [
        ("invalid.txt", vec![0xC0, 0xAF], true),
        ("binary.dat", vec![0, 1, 2], true),
        ("large.txt", b"public line\n".repeat(30_000), false),
        ("fake-large.png", b"public line\n".repeat(450_000), false),
    ] {
        fs::write(f.folder.join(name), bytes).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(f.folder.join(name))
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::new(1_700_000_000, 123_600_000),
            ))
            .unwrap();
        let preview = f.preview(name);
        assert_eq!(preview["modifiedAt"], "2023-11-14T22:13:20.123Z");
        assert_eq!(preview["binary"], binary, "{name}");
        if !binary {
            assert!(
                preview["text"]
                    .as_str()
                    .unwrap()
                    .starts_with("public line\n")
            );
            assert_eq!(preview["truncated"], true);
        }
    }
}
#[test]
fn same_bytes_replacement_changes_revision_and_rejects_stale_save() {
    let f = Fixture::new();
    fs::write(f.folder.join("a.txt"), "public").unwrap();
    let old = f.preview("a.txt");
    fs::rename(f.folder.join("a.txt"), f.folder.join("original.txt")).unwrap();
    fs::write(f.folder.join("a.txt"), "public").unwrap();
    let new = f.preview("a.txt");
    assert_ne!(old["revision"], new["revision"]);
    assert!(f.save("a.txt", "stale edit", &old).is_err());
    assert_eq!(
        fs::read_to_string(f.folder.join("a.txt")).unwrap(),
        "public"
    );
    let mut stale = new.clone();
    stale["bytes"] = json!(999);
    assert!(f.save("a.txt", "new edit", &stale).is_err());
    stale = new.clone();
    stale["modifiedAt"] = json!("1970-01-01T00:00:00.000Z");
    assert!(f.save("a.txt", "new edit", &stale).is_err());
}
#[test]
fn invalid_utf8_and_control_bytes_never_become_editable_text() {
    let f = Fixture::new();
    for (path, bytes) in [
        ("utf16.txt", vec![255, 254, 65, 0]),
        ("invalid.txt", vec![0xC0, 0xAF]),
        ("control.txt", vec![b'a', 1, b'b']),
    ] {
        fs::write(f.folder.join(path), bytes).unwrap();
        let p = f.preview(path);
        assert_eq!(p["binary"], true, "{p}");
        assert!(p["text"].is_null());
        assert!(p.get("revision").is_none());
    }
}
#[test]
fn oversized_first_line_has_bounded_noneditable_preview_and_save_budget() {
    let f = Fixture::new();
    fs::write(f.folder.join("large.txt"), "x".repeat(300 * 1024)).unwrap();
    let p = f.preview("large.txt");
    assert_eq!(p["truncated"], true);
    assert!(p["text"].is_null());
    assert!(p.get("revision").is_none());
    fs::write(f.folder.join("small.txt"), "small").unwrap();
    let p = f.preview("small.txt");
    assert!(
        f.save("small.txt", &"x".repeat(256 * 1024 + 1), &p)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.folder.join("small.txt")).unwrap(),
        "small"
    );
}
#[test]
fn raster_preview_is_validated_and_scaled_to_bounded_png() {
    let f = Fixture::new();
    let mut bytes = std::io::Cursor::new(vec![]);
    ::image::DynamicImage::new_rgb8(2000, 4)
        .write_to(&mut bytes, ::image::ImageFormat::Png)
        .unwrap();
    fs::write(f.folder.join("image.png"), bytes.get_ref()).unwrap();
    let p = f.preview("image.png");
    assert_eq!(p["imageMimeType"], "image/png");
    let png = STANDARD
        .decode(
            p["imageDataUrl"]
                .as_str()
                .unwrap()
                .strip_prefix("data:image/png;base64,")
                .unwrap(),
        )
        .unwrap();
    let decoded = ::image::load_from_memory(&png).unwrap();
    assert_eq!(decoded.width(), 1600);
    assert!(decoded.height() <= 1600);
    fs::write(f.folder.join("bad.png"), b"not an image").unwrap();
    let p = f.preview("bad.png");
    assert!(p.get("imageDataUrl").is_none());
    assert_eq!(p["binary"], true);
}
#[test]
fn pdf_header_and_payload_limits_are_enforced_before_publication() {
    let f = Fixture::new();
    let bytes = b"%PDF-1.7\npublic fixture\n%%EOF";
    fs::write(f.folder.join("public.pdf"), bytes).unwrap();
    let p = f.preview("public.pdf");
    assert_eq!(
        STANDARD
            .decode(p["pdfDataBase64"].as_str().unwrap())
            .unwrap(),
        bytes
    );
    fs::write(f.folder.join("bad.pdf"), b"arbitrary data").unwrap();
    assert!(f.preview("bad.pdf").get("pdfDataBase64").is_none());
    fs::File::create(f.folder.join("large.pdf"))
        .unwrap()
        .set_len(20 * 1024 * 1024 + 1)
        .unwrap();
    let p = f.preview("large.pdf");
    assert_eq!(p["truncated"], true);
    assert!(p.get("pdfDataBase64").is_none());
}
#[test]
fn create_and_rename_refuse_collisions_and_root_mutation() {
    let f = Fixture::new();
    assert_eq!(
        f.files
            .create(&f.projects, &f.roots, &f.id, "", "nested", "directory")
            .unwrap()["path"],
        "nested"
    );
    f.files
        .create(&f.projects, &f.roots, &f.id, "nested", "old.txt", "file")
        .unwrap();
    fs::write(f.folder.join("nested/old.txt"), "kept").unwrap();
    fs::write(f.folder.join("nested/occupied.txt"), "other").unwrap();
    assert!(
        f.files
            .rename(
                &f.projects,
                &f.roots,
                &f.id,
                "nested/old.txt",
                "occupied.txt"
            )
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.folder.join("nested/occupied.txt")).unwrap(),
        "other"
    );
    assert_eq!(
        f.files
            .rename(&f.projects, &f.roots, &f.id, "nested/old.txt", "New.txt")
            .unwrap()["path"],
        "nested/New.txt"
    );
    assert_eq!(
        fs::read_to_string(f.folder.join("nested/New.txt")).unwrap(),
        "kept"
    );
    assert!(
        f.files
            .rename(&f.projects, &f.roots, &f.id, "", "root-renamed")
            .is_err()
    );
    for name in ["", "..", "a/b", "a\\b"] {
        assert!(
            f.files
                .create(&f.projects, &f.roots, &f.id, "", name, "file")
                .is_err()
        );
    }
    #[cfg(windows)]
    {
        f.files
            .rename(&f.projects, &f.roots, &f.id, "nested/New.txt", "NEW.txt")
            .unwrap();
        assert!(
            fs::read_dir(f.folder.join("nested"))
                .unwrap()
                .any(|e| e.unwrap().file_name() == "NEW.txt")
        );
    }
}
#[test]
fn explorer_refuses_links_even_to_an_approved_sibling_and_honors_revocation() {
    let f = Fixture::new();
    let outside = f.root.path().join("sibling");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("public.txt"), "outside selected project").unwrap();
    let link = f.folder.join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    {
        let r = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
    assert!(
        f.files
            .preview(&f.projects, &f.roots, &f.id, "link/public.txt")
            .is_err()
    );
    assert!(
        f.files
            .create(&f.projects, &f.roots, &f.id, "link", "created.txt", "file")
            .is_err()
    );
    assert!(!outside.join("created.txt").exists());
    assert!(f.files.list(&f.projects, &[], &f.id, "").is_err());
}
