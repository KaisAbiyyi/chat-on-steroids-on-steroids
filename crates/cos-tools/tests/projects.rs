use cos_core::{config::Root, persistence::DurableStore};
use cos_tools::project::{self, Projects, Watch};
use std::{process::Command, time::Duration};
#[test]
fn persistent_catalog_git_and_native_watch() {
    let d = tempfile::tempdir().unwrap();
    let roots = [Root {
        name: "p".into(),
        path: std::fs::canonicalize(d.path())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }];
    let profile = tempfile::tempdir().unwrap();
    let durable = DurableStore::open(profile.path()).unwrap();
    let projects = Projects::open(durable.clone());
    let p = projects.add(&roots, "/p").unwrap();
    assert!(!p["id"].as_str().unwrap().is_empty());
    assert_eq!(projects.add(&roots, "/p").unwrap()["id"], p["id"]);
    projects
        .update(p["id"].as_str().unwrap(), None, Some(true))
        .unwrap();
    assert!(
        projects
            .add(&roots, "/p")
            .unwrap()
            .get("ungrouped")
            .is_none()
    );
    assert_eq!(Projects::open(durable).list().unwrap()[0]["id"], p["id"]);
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(d.path())
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(d.path().join("a file.txt"), "public fixture").unwrap();
    let git = project::git(&roots, "/p").unwrap();
    assert_eq!(git["status"], "ready");
    assert_eq!(git["files"][0]["path"], "a file.txt");
    let w = Watch::start(&roots, "/p").unwrap();
    std::fs::write(d.path().join("watch.txt"), "change").unwrap();
    let started = std::time::Instant::now();
    let mut paths = vec![];
    while started.elapsed() < Duration::from_secs(3) {
        let r = w.poll();
        paths.extend(r["paths"].as_array().unwrap().clone());
        if paths.iter().any(|v| v.as_str() == Some("/p/watch.txt")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(paths.iter().any(|v| v.as_str() == Some("/p/watch.txt")));
}
