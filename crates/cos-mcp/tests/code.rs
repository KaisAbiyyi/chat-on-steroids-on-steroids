use cos_mcp::code::{self, Limits};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
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
