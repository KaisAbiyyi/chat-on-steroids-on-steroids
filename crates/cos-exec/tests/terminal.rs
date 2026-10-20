use cos_exec::{
    shell,
    terminal::{Launch, Terminals},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
#[test]
fn closing_a_pending_terminal_prevents_launch() {
    let d = tempfile::tempdir().unwrap();
    let cwd = d.path().to_path_buf();
    let s = shell::detect(Some(if cfg!(windows) { "cmd" } else { "sh" })).unwrap();
    let gate = Arc::new(std::sync::Barrier::new(2));
    let resolve = gate.clone();
    let (started, ready) = std::sync::mpsc::channel();
    let validations = Arc::new(AtomicUsize::new(0));
    let checked = validations.clone();
    let terminals = Arc::new(Terminals::new(
        Arc::new(move |_| {
            started.send(()).unwrap();
            resolve.wait();
            Ok(Launch {
                program: s.path.clone(),
                args: s.args("echo effect > marker", false),
                cwd: cwd.clone(),
                shell: s.kind.clone(),
                identity: None,
            })
        }),
        Arc::new(|| true),
        Arc::new(move |_, _| {
            checked.fetch_add(1, Ordering::AcqRel);
            Ok(true)
        }),
        Arc::new(|_| {}),
    ));
    let opening = terminals.clone();
    let join = std::thread::spawn(move || opening.create("pending", None, 80, 24));
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    terminals.close("pending");
    gate.wait();
    assert!(join.join().unwrap().is_err());
    assert!(!d.path().join("marker").exists());
    assert_eq!(
        validations.load(Ordering::Acquire),
        0,
        "Cancelled open must stop before validation and spawning"
    );
}
#[test]
fn human_terminal_backpressure_is_bounded_and_permissions_are_live() {
    let d = tempfile::tempdir().unwrap();
    let cwd = d.path().to_path_buf();
    let s = shell::detect(Some(if cfg!(windows) { "cmd" } else { "sh" })).unwrap();
    let args = s.args(
        if cfg!(windows) {
            "for /l %i in (1,1,30000) do @echo public-1234567890"
        } else {
            "i=0; while [ $i -lt 30000 ]; do echo public-1234567890; i=$((i+1)); done"
        },
        false,
    );
    let permitted = Arc::new(AtomicBool::new(true));
    let allowed = permitted.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let read = count.clone();
    let ended = done.clone();
    let terminals = Terminals::new(
        Arc::new(move |_| {
            Ok(Launch {
                program: s.path.clone(),
                args: args.clone(),
                cwd: cwd.clone(),
                shell: s.kind.clone(),
                identity: None,
            })
        }),
        Arc::new(move || allowed.load(Ordering::Acquire)),
        Arc::new(|_, _| Ok(true)),
        Arc::new(move |event| {
            if let Some(data) = event["data"].as_str() {
                read.fetch_add(data.encode_utf16().count(), Ordering::AcqRel);
            }
            if event["exitCode"].is_number() {
                ended.store(true, Ordering::Release);
            }
        }),
    );
    let info = terminals.create("renderer-terminal", None, 80, 24).unwrap();
    assert_eq!(info["id"], "renderer-terminal");
    let start = Instant::now();
    while count.load(Ordering::Acquire) < 262144 && start.elapsed() < Duration::from_secs(4) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(count.load(Ordering::Acquire) >= 262144);
    std::thread::sleep(Duration::from_millis(60));
    assert!(count.load(Ordering::Acquire) < 300000);
    permitted.store(false, Ordering::Release);
    assert_eq!(
        terminals.write("renderer-terminal", "x").unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    // Resize used to bypass the live project/user permission check.
    assert_eq!(
        terminals.resize("renderer-terminal", 90, 30).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    permitted.store(true, Ordering::Release);
    assert_eq!(
        terminals.resize("renderer-terminal", 0, 24).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        terminals.resize("renderer-terminal", 1001, 24).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    let mut acked = 0;
    let start = Instant::now();
    while !done.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(8) {
        let now = count.load(Ordering::Acquire);
        terminals.acknowledge("renderer-terminal", now - acked);
        acked = now;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(done.load(Ordering::Acquire));
    assert!(count.load(Ordering::Acquire) > 500000);
    terminals.dispose();
    assert!(terminals.exited(Duration::from_secs(3)));
}

#[test]
fn invalid_terminal_dimensions_are_rejected_before_factory_or_spawn() {
    let called = Arc::new(AtomicUsize::new(0));
    let gate = called.clone();
    let terminals = Terminals::new(
        Arc::new(move |_| {
            gate.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::other("factory must not run"))
        }),
        Arc::new(|| true),
        Arc::new(|_, _| Ok(true)),
        Arc::new(|_| {}),
    );
    for (cols, rows) in [(0, 24), (80, 0), (1001, 24), (80, 501)] {
        assert_eq!(
            terminals.create("bad-dimensions", None, cols, rows).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    assert_eq!(called.load(Ordering::SeqCst), 0);
}
