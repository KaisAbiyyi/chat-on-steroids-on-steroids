use crate::code::{Invoke, Limits};
use cos_exec::tree::Tree;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const FRAME: usize = 12 * 1024 * 1024 + 65536;
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}
fn read_frame(reader: &mut impl BufRead) -> io::Result<Value> {
    let mut bytes = vec![];
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Code worker closed",
            ));
        }
        let end = chunk
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(chunk.len());
        if bytes.len() + end > FRAME {
            return Err(io::Error::other("Code worker frame exceeds limit"));
        }
        let done = chunk[end - 1] == b'\n';
        bytes.extend_from_slice(&chunk[..end]);
        reader.consume(end);
        if done {
            return serde_json::from_slice(&bytes).map_err(io::Error::other);
        }
    }
}
fn write_frame(writer: &mut impl Write, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() > FRAME {
        return Err(io::Error::other("Code worker frame exceeds limit"));
    }
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}
pub fn run(
    executable: &Path,
    code: &str,
    tools: &[Value],
    invoke: Invoke,
    limits: Limits,
) -> Value {
    if ACTIVE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            if n < 4 { Some(n + 1) } else { None }
        })
        .is_err()
    {
        return crate::code::error("BUSY");
    }
    let permit = Arc::new(Permit);
    let mut command = Command::new(executable);
    command
        .arg("--code-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000004);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => return crate::code::error("RUNTIME_UNAVAILABLE"),
    };
    let tree = match Tree::attach(child.id(), cfg!(windows)) {
        Ok(t) => t,
        Err(_) => {
            let _ = child.kill();
            return crate::code::error("RUNTIME_UNAVAILABLE");
        }
    };
    if tree.memory_limit(128 * 1024 * 1024).is_err() {
        tree.terminate();
        let _ = child.wait();
        return crate::code::error("RUNTIME_UNAVAILABLE");
    }
    let input = Arc::new(Mutex::new(child.stdin.take().unwrap()));
    let descriptions = tools
        .iter()
        .map(|t| json!({"name":t["name"],"description":t["description"]}))
        .collect::<Vec<_>>();
    if write_frame(
        &mut *input.lock().unwrap(),
        &json!({"code":code,"tools":descriptions,"limits":limits}),
    )
    .is_err()
    {
        tree.terminate();
        let _ = child.wait();
        return crate::code::error("BRIDGE_ERROR");
    }
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let frame = read_frame(&mut reader);
            let done = frame.is_err();
            if tx.send(frame).is_err() || done {
                break;
            }
        }
    });
    let started = Instant::now();
    let allowed = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let pending = Arc::new(AtomicUsize::new(0));
    let mut calls = 0;
    let mut emissions = vec![];
    let mut bytes = 0;
    let result = loop {
        let remaining = Duration::from_millis(limits.wall_ms.saturating_add(500))
            .saturating_sub(started.elapsed());
        let frame = match rx.recv_timeout(remaining) {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => break crate::code::error("RUNTIME_ERROR"),
            Err(_) => break crate::code::error("TIME_LIMIT"),
        };
        match frame["type"].as_str() {
            Some("call") => {
                let Some(name) = frame["name"].as_str() else {
                    break crate::code::error("BRIDGE_ERROR");
                };
                if !allowed.contains(name)
                    || matches!(name, "exec" | "session_finish")
                    || calls >= 32
                    || pending.load(Ordering::Acquire) >= 8
                    || frame["args"].to_string().len() > 1024 * 1024
                {
                    break crate::code::error("CALL_LIMIT");
                }
                calls += 1;
                pending.fetch_add(1, Ordering::AcqRel);
                let work = invoke.clone();
                let name = name.to_string();
                let args = frame["args"].clone();
                let id = frame["id"].clone();
                let writer = input.clone();
                let pending = pending.clone();
                let active = permit.clone();
                std::thread::spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        work(&name, args)
                    }))
                    .unwrap_or_else(|_| crate::code::error("TOOL_ERROR"));
                    let result = if result.to_string().len() > 12 * 1024 * 1024 {
                        crate::code::error("RESULT_LIMIT")
                    } else {
                        result
                    };
                    let _ = write_frame(
                        &mut *writer.lock().unwrap(),
                        &json!({"type":"result","id":id,"result":result}),
                    );
                    pending.fetch_sub(1, Ordering::AcqRel);
                    drop(active);
                });
            }
            Some("emit") => {
                let item = frame["item"].clone();
                bytes += item.to_string().len();
                if emissions.len() >= 32 || bytes > 12 * 1024 * 1024 {
                    break crate::code::error("OUTPUT_LIMIT");
                }
                emissions.push(item);
            }
            Some("done") => break frame["result"].clone(),
            _ => break crate::code::error("BRIDGE_ERROR"),
        }
    };
    tree.terminate();
    let _ = child.kill();
    let _ = child.wait();
    if result["isError"] == true
        && result["content"].as_array().is_some_and(|a| a.len() == 1)
        && !emissions.is_empty()
    {
        emissions.extend(result["content"].as_array().unwrap().clone());
        json!({"content":emissions,"isError":true})
    } else {
        result
    }
}
/// Hidden executable entry: no profile, network, filesystem or application bootstrap.
pub fn entry() -> io::Result<()> {
    #[cfg(unix)]
    unsafe {
        let memory = libc::rlimit {
            rlim_cur: 128 * 1024 * 1024,
            rlim_max: 128 * 1024 * 1024,
        };
        let no_core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_AS, &memory) != 0
            || libc::setrlimit(libc::RLIMIT_CORE, &no_core) != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    let spec = read_frame(&mut BufReader::new(io::stdin().lock()))?;
    let code = spec["code"]
        .as_str()
        .ok_or_else(|| io::Error::other("Missing worker code"))?
        .to_string();
    let tools = spec["tools"]
        .as_array()
        .ok_or_else(|| io::Error::other("Missing worker tools"))?
        .clone();
    let limits: Limits =
        serde_json::from_value(spec["limits"].clone()).map_err(io::Error::other)?;
    let writer = Arc::new(Mutex::new(io::stdout()));
    let waiting = Arc::new(Mutex::new(BTreeMap::<u64, mpsc::SyncSender<Value>>::new()));
    let receive = waiting.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(io::stdin());
        while let Ok(frame) = read_frame(&mut reader) {
            if let Some(id) = frame["id"].as_u64()
                && let Some(tx) = receive.lock().unwrap().remove(&id)
            {
                let _ = tx.send(frame["result"].clone());
            }
        }
        receive.lock().unwrap().clear();
    });
    let next = Arc::new(std::sync::atomic::AtomicU64::new(1));
    let output = writer.clone();
    let invoke: Invoke = Arc::new(move |name, args| {
        let id = next.fetch_add(1, Ordering::AcqRel);
        let (tx, rx) = mpsc::sync_channel(1);
        waiting.lock().unwrap().insert(id, tx);
        if write_frame(
            &mut *output.lock().unwrap(),
            &json!({"type":"call","id":id,"name":name,"args":args}),
        )
        .is_err()
        {
            return crate::code::error("BRIDGE_ERROR");
        }
        rx.recv_timeout(Duration::from_millis(limits.wall_ms))
            .unwrap_or_else(|_| crate::code::error("TIME_LIMIT"))
    });
    let output = writer.clone();
    let notify = Arc::new(move |item| {
        let _ = write_frame(
            &mut *output.lock().unwrap(),
            &json!({"type":"emit","item":item}),
        );
    });
    let result = crate::code::run_notifying(&code, &tools, invoke, limits, Some(notify));
    write_frame(
        &mut *writer.lock().unwrap(),
        &json!({"type":"done","result":result}),
    )
}
