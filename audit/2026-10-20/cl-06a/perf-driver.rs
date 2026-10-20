use cos_core::config::Root;
use std::{fs, process::Command, time::Instant};
fn git(path: &std::path::Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=Public Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgSign=false", "-c", "core.hooksPath=/dev/null"]).args(args).current_dir(path).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
fn main() {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q", "-b", "main"]);
    for n in 0..64 { fs::write(d.path().join(format!("file-{n:03}.txt")), "base line\n".repeat(400)).unwrap(); }
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-q", "-m", "public perf fixture"]);
    for n in 0..64 { fs::write(d.path().join(format!("file-{n:03}.txt")), "edit line\n".repeat(400)).unwrap(); }
    let roots = [Root { name:"p".into(), path:fs::canonicalize(d.path()).unwrap().to_string_lossy().into(), remote:None }];
    let mut snapshots = vec![];
    let mut diffs = vec![];
    for n in 0..12 {
        let start = Instant::now();
        let s = cos_tools::git::snapshot(&roots, "/p", "fixture", None).unwrap();
        assert_eq!(s["state"], "ready");
        if n >= 2 { snapshots.push(start.elapsed().as_secs_f64() * 1000.); }
        let start = Instant::now();
        let diff = cos_tools::git::diff(&roots, "/p", "fixture", "file-000.txt", None, s["revision"].as_str()).unwrap();
        assert!(diff["currentText"].as_str().unwrap().starts_with("edit line\n"));
        if n >= 2 { diffs.push(start.elapsed().as_secs_f64() * 1000.); }
    }
    snapshots.sort_by(f64::total_cmp); diffs.sort_by(f64::total_cmp);
    println!("{{\"fixture\":\"64 modified UTF-8 files x 4000 bytes\",\"samples\":10,\"warmups\":2,\"snapshot_ms_p50\":{},\"snapshot_ms_p95\":{},\"diff_ms_p50\":{},\"diff_ms_p95\":{}}}", snapshots[4], snapshots[9], diffs[4], diffs[9]);
}
