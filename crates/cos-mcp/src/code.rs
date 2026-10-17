use rquickjs::{Context, Function, Persistent, Runtime, function::Func};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
pub type Invoke = Arc<dyn Fn(&str, Value) -> Value + Send + Sync>;
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Limits {
    pub cpu_ms: u64,
    pub wall_ms: u64,
    pub memory: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            cpu_ms: 2000,
            wall_ms: 60000,
            memory: 32 * 1024 * 1024,
        }
    }
}
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}
#[derive(Default)]
struct State {
    queue: VecDeque<(u32, String, Value)>,
    calls: usize,
    pending: usize,
    content: Vec<Value>,
    text_bytes: usize,
    output_bytes: usize,
    images: usize,
    failure: Option<String>,
    done: bool,
    result_bytes: usize,
}
struct Cpu {
    spent: Duration,
    since: Instant,
}
pub fn error(reason: impl Into<String>) -> Value {
    json!({"content":[{"type":"text","text":format!("CODE_MODE_{}",reason.into())}],"isError":true})
}
fn js_error() -> rquickjs::Error {
    rquickjs::Error::new_from_js_message(
        "host",
        "bridge",
        "Code mode boundary refused this operation",
    )
}
pub type Notify = Arc<dyn Fn(Value) + Send + Sync>;
pub fn run(code: &str, tools: &[Value], invoke: Invoke, limits: Limits) -> Value {
    run_notifying(code, tools, invoke, limits, None)
}
pub fn run_notifying(
    code: &str,
    tools: &[Value],
    invoke: Invoke,
    limits: Limits,
    notify: Option<Notify>,
) -> Value {
    if code.trim().is_empty() || cos_core::config::js_len(code) > 64000 {
        return error("CODE_LIMIT");
    }
    if code
        .lines()
        .any(|l| l.trim_start().starts_with("// @exec:"))
    {
        return error(
            "UNSUPPORTED_PRAGMA: use plain JavaScript; execution is bounded to one call.",
        );
    }
    if ACTIVE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
            if v < 4 { Some(v + 1) } else { None }
        })
        .is_err()
    {
        return error("BUSY: too many code executions or unsettled nested calls.");
    }
    let permit = Arc::new(Permit);
    let state = Arc::new(Mutex::new(State::default()));
    let allowed = tools
        .iter()
        .filter_map(|v| v["name"].as_str())
        .filter(|s| !matches!(*s, "exec" | "session_finish"))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return error("RUNTIME_ERROR"),
    };
    runtime.set_memory_limit(limits.memory);
    runtime.set_max_stack_size(256 * 1024);
    let cpu = Arc::new(Mutex::new(Cpu {
        spent: Duration::ZERO,
        since: Instant::now(),
    }));
    let clock = cpu.clone();
    let started = Instant::now();
    let boundary = state.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let c = clock.lock().unwrap();
        let over = c.spent + c.since.elapsed() > Duration::from_millis(limits.cpu_ms)
            || started.elapsed() > Duration::from_millis(limits.wall_ms);
        if over && let Ok(mut s) = boundary.lock() {
            s.failure = Some("TIME_LIMIT".into());
        }
        over
    })));
    let context = match Context::full(&runtime) {
        Ok(c) => c,
        Err(_) => return error("RUNTIME_ERROR"),
    };
    let setup=context.with(|ctx|->rquickjs::Result<_>{
  let calls=state.clone();ctx.globals().set("__call",Func::from(move|id:u32,name:String,args:String|->rquickjs::Result<()>{let mut s=calls.lock().unwrap();if s.failure.is_some(){return Err(js_error());}let refusal=if !allowed.contains(&name){Some("UNKNOWN_TOOL")}else if s.calls>=32||s.pending>=8||args.len()>1024*1024{Some("CALL_LIMIT")}else{None};if let Some(reason)=refusal{s.failure=Some(reason.into());return Err(js_error());}let args=serde_json::from_str(&args).map_err(|_|js_error())?;s.calls+=1;s.pending+=1;s.queue.push_back((id,name,args));Ok(())}))?;
  let emits=state.clone();ctx.globals().set("__emit",Func::from(move|kind:String,value:String|->rquickjs::Result<()>{let mut s=emits.lock().unwrap();if s.failure.is_some(){return Err(js_error());}
  if s.content.len()>=32||s.output_bytes+value.len()>12*1024*1024{s.failure=Some("OUTPUT_LIMIT".into());return Err(js_error());}let value:Value=serde_json::from_str(&value).map_err(|_|js_error())?;if kind=="text"{let value=value.as_str().ok_or_else(js_error)?;let remaining=40000usize.saturating_sub(s.text_bytes);if value.len()>remaining{let mut end=remaining;while !value.is_char_boundary(end){end-=1;}
  if end>0{let item=json!({"type":"text","text":&value[..end]});if let Some(notify)=&notify{notify(item.clone());}s.content.push(item);}s.failure=Some("OUTPUT_LIMIT".into());return Err(js_error());}s.text_bytes+=value.len();s.output_bytes+=value.len();s.content.push(json!({"type":"text","text":value}));}else if kind=="image"{
   if s.images>=4{s.failure=Some("OUTPUT_LIMIT".into());return Err(js_error());}let item=crate::image_content(&value).map_err(|_|{s.failure=Some("IMAGE_INVALID".into());js_error()})?;s.images+=1;s.output_bytes+=item.to_string().len();s.content.push(item);
  }else{s.failure=Some("OUTPUT_INVALID".into());return Err(js_error());}
  if let Some(notify)=&notify{notify(s.content.last().unwrap().clone());}Ok(())}))?;
  let done=state.clone();ctx.globals().set("__done",Func::from(move|reason:Option<String>|{let mut s=done.lock().unwrap();s.done=true;if let Some(reason)=reason&&s.failure.is_none(){s.failure=Some(format!("SCRIPT_ERROR: {}",reason.chars().take(1000).collect::<String>()));}}))?;
  ctx.globals().set("__tools_json",serde_json::to_string(tools).unwrap())?;
  let resolve=ctx.eval::<Function,_>(r#"((call,emit,descriptions)=>{let next=0;const pending=new Map();const tools=Object.create(null);const list=JSON.parse(descriptions);for(const tool of list){if(tool.name==='exec'||tool.name==='session_finish')continue;tools[tool.name]=(args={})=>new Promise((resolve,reject)=>{const id=++next;pending.set(id,{resolve,reject});try{call(id,tool.name,JSON.stringify(args));}catch(e){pending.delete(id);reject(e);}});}globalThis.tools=Object.freeze(tools);globalThis.ALL_TOOLS=Object.freeze(list.map(x=>Object.freeze({name:x.name,description:x.description})));globalThis.text=v=>emit('text',JSON.stringify(typeof v==='string'?v:(JSON.stringify(v)??String(v))));globalThis.image=v=>emit('image',JSON.stringify(v));return(id,json)=>{const p=pending.get(id);if(p){pending.delete(id);p.resolve(JSON.parse(json));}};})(__call,__emit,__tools_json)"#)?;
  ctx.eval::<(),_>("delete globalThis.__call;delete globalThis.__emit;delete globalThis.__tools_json;")?;
  ctx.eval::<(),_>(format!("((done)=>{{delete globalThis.__done;(async()=>{{\n{code}\n}})().then(()=>done(null),e=>done(String(e)));}})(__done)"))?;
  Ok(Persistent::save(&ctx,resolve))
 });
    let resolve = match setup {
        Ok(r) => Some(r),
        Err(_) => {
            let mut s = state.lock().unwrap();
            if s.failure.is_none() {
                s.failure = Some("RUNTIME_ERROR".into());
            }
            None
        }
    };
    {
        let mut c = cpu.lock().unwrap();
        c.spent = c.since.elapsed();
    }
    let (tx, rx) = mpsc::channel::<(u32, Value)>();
    loop {
        {
            let mut s = state.lock().unwrap();
            if started.elapsed() > Duration::from_millis(limits.wall_ms) && s.failure.is_none() {
                s.failure = Some("TIME_LIMIT".into());
            }
            if s.failure.is_some() {
                break;
            }
            if s.done {
                if s.pending > 0 {
                    s.failure = Some("UNAWAITED_CALLS: dispatched actions may still finish".into());
                }
                break;
            }
        }
        let queued = { state.lock().unwrap().queue.drain(..).collect::<Vec<_>>() };
        for (id, name, args) in queued {
            let call = invoke.clone();
            let sender = tx.clone();
            let active = permit.clone();
            thread::spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&name, args)))
                        .unwrap_or_else(|_| error("TOOL_ERROR"));
                let _ = sender.send((id, result));
                drop(active);
            });
        }
        while let Ok((id, result)) = rx.try_recv() {
            let json = serde_json::to_string(&result).unwrap_or_else(|_| "null".into());
            {
                let mut s = state.lock().unwrap();
                s.pending = s.pending.saturating_sub(1);
                s.result_bytes += json.len();
                if json.len() > 12 * 1024 * 1024 || s.result_bytes > 32 * 1024 * 1024 {
                    s.failure = Some("RESULT_LIMIT".into());
                    break;
                }
            }
            if let Some(resolve) = &resolve {
                cpu.lock().unwrap().since = Instant::now();
                let result =
                    context.with(|ctx| resolve.clone().restore(&ctx)?.call::<_, ()>((id, json)));
                let elapsed = cpu.lock().unwrap().since.elapsed();
                cpu.lock().unwrap().spent += elapsed;
                if result.is_err() {
                    state.lock().unwrap().failure = Some("BRIDGE_ERROR".into());
                }
            }
        }
        cpu.lock().unwrap().since = Instant::now();
        let job = runtime.execute_pending_job();
        let elapsed = cpu.lock().unwrap().since.elapsed();
        cpu.lock().unwrap().spent += elapsed;
        match job {
            Ok(true) => {}
            Ok(false) => thread::sleep(Duration::from_millis(1)),
            Err(_) => {
                let mut s = state.lock().unwrap();
                if s.failure.is_none() {
                    s.failure = Some("RUNTIME_ERROR".into());
                }
                break;
            }
        }
    }
    // Drop all persistent JS values before their runtime. Accepted native calls keep
    // their own lifetime and active-run permit until their real result is recorded.
    drop(resolve);
    let mut s = state.lock().unwrap();
    let mut content = std::mem::take(&mut s.content);
    if let Some(reason) = &s.failure {
        content.push(json!({"type":"text","text":format!("CODE_MODE_{reason}")}));
        json!({"content":content,"isError":true})
    } else {
        json!({"content":content})
    }
}
