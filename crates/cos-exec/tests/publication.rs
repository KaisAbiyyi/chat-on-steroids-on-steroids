use cos_exec::{
    process::{Launch, Manager, PublicationScope},
    shell,
};
use std::time::Duration;
#[test]
fn aborted_publication_restores_unread_output_without_rerunning() {
    let d = tempfile::tempdir().unwrap();
    let manager = Manager::default();
    let s = shell::detect(Some(if cfg!(windows) { "cmd" } else { "sh" })).unwrap();
    let args = s.args("echo public-once", false);
    let scope = PublicationScope::begin();
    let result = manager
        .launch(
            Launch {
                owner: "session:public",
                program: &s.path,
                args: &args,
                cwd: d.path(),
                tty: false,
                raw_cmd: s.kind == "cmd",
            },
            Duration::from_secs(2),
        )
        .unwrap();
    let id = result["completed_session_id"].as_i64().unwrap() as i32;
    let delivery = scope.finish();
    drop(delivery);
    assert_eq!(
        manager.snapshot("session:public").unwrap()[0]["unread"],
        true
    );
    let retry = manager
        .poll(id, "session:public", "", Duration::ZERO)
        .unwrap();
    assert!(retry["output"].as_str().unwrap().contains("public-once"));
    assert_ne!(retry["output_replayed"], true);
    assert_eq!(
        manager.snapshot("session:public").unwrap()[0]["unread"],
        false
    );
}
#[test]
fn committed_publication_marks_the_result_read() {
    let d = tempfile::tempdir().unwrap();
    let manager = Manager::default();
    let s = shell::detect(Some(if cfg!(windows) { "cmd" } else { "sh" })).unwrap();
    let args = s.args("echo public-commit", false);
    let scope = PublicationScope::begin();
    let result = manager
        .launch(
            Launch {
                owner: "session:public",
                program: &s.path,
                args: &args,
                cwd: d.path(),
                tty: false,
                raw_cmd: s.kind == "cmd",
            },
            Duration::from_secs(2),
        )
        .unwrap();
    let mut publication = scope.finish();
    let id = result["completed_session_id"].as_i64().unwrap() as i32;
    assert!(
        manager
            .poll(
                id,
                "session:public",
                "input-must-not-be-sent",
                Duration::ZERO
            )
            .unwrap_err()
            .to_string()
            .contains("EXEC_OUTPUT_PENDING")
    );
    publication.commit();
    drop(publication);
    assert_eq!(
        manager.snapshot("session:public").unwrap()[0]["unread"],
        false
    );
}
