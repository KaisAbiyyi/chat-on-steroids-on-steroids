use cos_mcp::code::{self, Limits};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
// The embedded engine has a deliberate process-wide four-run admission cap.
// A parallel integration-test harness must not consume unrelated tests\' permits.
static CODE_TEST_MUTEX: Mutex<()> = Mutex::new(());
fn invoke() -> code::Invoke {
    Arc::new(
        |name, args| json!({"content":[{"type":"text","text":format!("{name}:{}",args["text"].as_str().unwrap_or(""))}]}),
    )
}
fn tools() -> Vec<Value> {
    vec![json!({"name":"read","description":"fixture read"})]
}
#[test]
fn explicit_outputs_only_fresh_globals() {
    let _serial = CODE_TEST_MUTEX.lock().unwrap();
    let result = code::run(
        "const r=await tools.read({text:'hello'}); text(r.content[0].text); globalThis.secret=123;",
        &tools(),
        invoke(),
        Limits::default(),
    );
    assert_eq!(result["content"][0]["text"], "read:hello");
    assert_eq!(
        code::run(
            "text(typeof secret);",
            &tools(),
            invoke(),
            Limits::default()
        )["content"][0]["text"],
        "undefined"
    );
    assert_eq!(
        code::run(
            "await tools.read({text:'hidden'});",
            &tools(),
            invoke(),
            Limits::default()
        )["content"],
        json!([])
    );
}
#[test]
fn no_node_network_or_cross_surface_tools() {
    let _serial = CODE_TEST_MUTEX.lock().unwrap();
    let r = code::run(
        "text([typeof process,typeof require,typeof fetch,typeof console,typeof setTimeout,typeof tools.computer]);",
        &tools(),
        invoke(),
        Limits::default(),
    );
    assert_eq!(
        r["content"][0]["text"],
        "[\"undefined\",\"undefined\",\"undefined\",\"undefined\",\"undefined\",\"undefined\"]"
    );
    let r = code::run(
        "await tools.computer({});",
        &tools(),
        invoke(),
        Limits::default(),
    );
    assert_eq!(r["isError"], true);
}
#[test]
fn cpu_memory_calls_and_text_limits() {
    let _serial = CODE_TEST_MUTEX.lock().unwrap();
    let limits = Limits {
        cpu_ms: 30,
        wall_ms: 150,
        memory: 32 * 1024 * 1024,
    };
    let start = std::time::Instant::now();
    let r = code::run("while(true){}", &tools(), invoke(), limits);
    assert_eq!(r["isError"], true);
    assert!(start.elapsed().as_millis() < 1000);
    let r = code::run("text('x'.repeat(40001));", &tools(), invoke(), limits);
    assert_eq!(r["isError"], true);
    assert_eq!(r["content"][0]["text"].as_str().unwrap().len(), 40000);
    let r = code::run(
        "let a=[]; while(true) a.push(new Array(10000).fill('x'));",
        &tools(),
        invoke(),
        Limits {
            cpu_ms: 2000,
            wall_ms: 3000,
            memory: 1024 * 1024,
        },
    );
    assert_eq!(r["isError"], true);
}
#[test]
fn accepted_side_effect_survives_timeout() {
    let _serial = CODE_TEST_MUTEX.lock().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let r = code::run(
        "await tools.read({}); while(true){}",
        &tools(),
        Arc::new(move |_, _| {
            c.fetch_add(1, Ordering::AcqRel);
            json!({"content":[]})
        }),
        Limits {
            cpu_ms: 30,
            wall_ms: 150,
            memory: 32 * 1024 * 1024,
        },
    );
    assert_eq!(r["isError"], true);
    assert_eq!(calls.load(Ordering::Acquire), 1);
}

#[test]
fn rejects_invalid_resource_admission_before_invoking_tools() {
    let _serial = CODE_TEST_MUTEX.lock().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    for limits in [
        Limits {
            cpu_ms: 0,
            ..Limits::default()
        },
        Limits {
            wall_ms: 0,
            ..Limits::default()
        },
        Limits {
            cpu_ms: 60_001,
            ..Limits::default()
        },
        Limits {
            wall_ms: 60_001,
            ..Limits::default()
        },
        Limits {
            memory: 1024,
            ..Limits::default()
        },
        Limits {
            memory: 65 * 1024 * 1024,
            ..Limits::default()
        },
    ] {
        let counter = calls.clone();
        let result = code::run(
            "await tools.read({text:'should-not-execute'});",
            &tools(),
            Arc::new(move |_, _| {
                counter.fetch_add(1, Ordering::AcqRel);
                json!({"content":[]})
            }),
            limits,
        );
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["text"], "CODE_MODE_RESOURCE_LIMIT");
    }
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

#[test]
fn nested_call_admission_blocks_unawaited_and_excessive_calls() {
    let _serial = CODE_TEST_MUTEX.lock().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let observed = seen.clone();
    let unawaited = code::run(
        "tools.read({text:'accepted'});",
        &tools(),
        Arc::new(move |_, _| {
            // Preserve a genuinely in-flight mutation when the unawaited
            // script completes. An instant callback races the JS completion
            // job and does not test the unsettled-call contract reliably.
            std::thread::sleep(std::time::Duration::from_millis(50));
            observed.fetch_add(1, Ordering::AcqRel);
            json!({"content":[]})
        }),
        Limits::default(),
    );
    assert_eq!(unawaited["isError"], true);
    assert!(unawaited["content"].as_array().unwrap().iter().any(|item| {
        item["text"]
            .as_str()
            .unwrap_or("")
            .contains("UNAWAITED_CALLS")
    }));
    // A completed script must not represent dispatched, unsettled mutations as success.
    let overflow = code::run(
        "await Promise.all(Array.from({length: 33}, () => tools.read({text:'many'})));",
        &tools(),
        invoke(),
        Limits::default(),
    );
    assert_eq!(overflow["isError"], true);
    // The admitted external mutation may outlive the unawaited script.
    // Do not release this test's global admission lock until that worker
    // finishes and releases its process-wide permit.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while seen.load(Ordering::Acquire) == 0 {
        assert!(std::time::Instant::now() < deadline, "nested fixture did not settle");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    std::thread::sleep(std::time::Duration::from_millis(15));
}
