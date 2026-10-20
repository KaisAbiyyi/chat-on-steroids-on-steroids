use cos_core::{
    logging::DiagnosticLog,
    runtime::{Cancellation, ShutdownPhase, shutdown},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[test]
fn diagnostic_secrets_and_size_are_bounded() {
    let log = DiagnosticLog::new(2);
    let secret = format!("sk-{}", "A".repeat(40));
    log.push("info", &format!("{}{}", "p".repeat(15995), secret));
    let first = log.snapshot();
    let output = first[0]["message"].as_str().unwrap();
    assert!(!output.contains(&secret));
    assert!(output.chars().count() <= 16000);
    log.push(&format!("error{}", secret), &format!("{}{}", secret, "x".repeat(1024 * 1024)));
    let second = log.snapshot();
    assert_eq!(second[1]["level"], "unknown");
    assert_eq!(second[1]["message"], "[diagnostic omitted: size limit]");
    assert!(!log.export().contains(&secret));
    log.push("error", "safe");
    assert_eq!(log.snapshot().len(), 2);
}

#[test]
fn shutdown_continues_after_panic_and_sets_cancel() {
    let cancel = Cancellation::default();
    let reached = Arc::new(AtomicBool::new(false));
    let seen = reached.clone();
    let report = shutdown(
        &cancel,
        vec![
            ShutdownPhase::new("panic", Duration::from_secs(1), || {
                panic!("synthetic phase panic");
            }),
            ShutdownPhase::new("flush", Duration::from_secs(1), move || {
                seen.store(true, Ordering::SeqCst);
                Ok(())
            }),
        ],
    );
    assert!(cancel.is_cancelled());
    assert_eq!(report[0].status, "failed");
    assert_eq!(report[1].status, "ok");
    assert!(reached.load(Ordering::SeqCst));
}

#[test]
fn timeout_still_attempts_later_phases() {
    let cancel = Cancellation::default();
    let started = Instant::now();
    let report = shutdown(
        &cancel,
        vec![
            ShutdownPhase::new("slow", Duration::from_millis(5), || {
                std::thread::sleep(Duration::from_millis(200));
                Ok(())
            }),
            ShutdownPhase::new("next", Duration::from_secs(1), || Ok(())),
        ],
    );
    assert_eq!(report[0].status, "timeout");
    assert_eq!(report[1].status, "ok");
    assert!(started.elapsed() < Duration::from_millis(150));
}
