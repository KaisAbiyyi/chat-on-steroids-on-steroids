//! Source-aligned, privacy-bounded activity captions.
//! Captions are pure projections of CL-01 sanitized args and CL-02 trusted evidence.
//! Never guess a command exit code or changes from console text.
use serde_json::{Value, json};

struct Phrase {
    kind: &'static str,
    past: String,
    doing: String,
    infinitive: String,
    tone: &'static str,
    detail: Option<String>,
    metric: Option<String>,
    special_error: bool,
}

fn path(value: &Value) -> String {
    let text = value.as_str().unwrap_or("/");
    let parts: Vec<_> = text.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => "/".into(),
        1 => parts[0].to_string(),
        _ => parts[1..].join("/"),
    }
}
fn quote(value: &Value) -> String {
    let s = value.as_str().unwrap_or("");
    let truncated = s.chars().take(60).collect::<String>();
    format!(
        "\"{}\"",
        truncated.replace('\\', "\\\\").replace('"', "\\\"")
    )
}
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
fn counter(value: &Value) -> Option<usize> {
    value.as_u64().and_then(|n| usize::try_from(n).ok())
}
fn phrase(
    kind: &'static str,
    past: impl Into<String>,
    doing: impl Into<String>,
    infinitive: impl Into<String>,
) -> Phrase {
    Phrase {
        kind,
        past: past.into(),
        doing: doing.into(),
        infinitive: infinitive.into(),
        tone: "neutral",
        detail: None,
        metric: None,
        special_error: false,
    }
}
fn detail_count(value: &Value, singular: &str, plural_word: &str) -> Option<String> {
    counter(value).map(|n| plural(n, singular, plural_word))
}
fn verb(name: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match name {
        "list_windows" => (
            "screen",
            "Listed open windows",
            "Listing open windows",
            "list open windows",
        ),
        "get_window" => (
            "screen",
            "Inspected a window",
            "Inspecting a window",
            "inspect a window",
        ),
        "list_apps" => (
            "screen",
            "Listed installed apps",
            "Listing installed apps",
            "list installed apps",
        ),
        "get_window_state" => (
            "screen",
            "Inspected window state",
            "Inspecting window state",
            "inspect window state",
        ),
        "launch_app" => (
            "input",
            "Requested an app launch",
            "Requesting an app launch",
            "request an app launch",
        ),
        "click" => (
            "input",
            "Clicked in a window",
            "Clicking in a window",
            "click in a window",
        ),
        "press_key" => (
            "input",
            "Pressed keys in a window",
            "Pressing keys in a window",
            "press keys in a window",
        ),
        "type_text" => (
            "input",
            "Typed in a window",
            "Typing in a window",
            "type in a window",
        ),
        "scroll" => (
            "input",
            "Scrolled in a window",
            "Scrolling in a window",
            "scroll in a window",
        ),
        "set_value" => (
            "input",
            "Replaced a field value",
            "Replacing a field value",
            "replace a field value",
        ),
        "drag" => (
            "input",
            "Dragged in a window",
            "Dragging in a window",
            "drag in a window",
        ),
        "perform_secondary_action" => (
            "input",
            "Acted on a window control",
            "Acting on a window control",
            "act on a window control",
        ),
        "activate_window" => (
            "input",
            "Focused a window",
            "Focusing a window",
            "focus a window",
        ),
        "read_clipboard" => (
            "clipboard",
            "Read the clipboard",
            "Reading the clipboard",
            "read the clipboard",
        ),
        "write_clipboard" => (
            "clipboard",
            "Replaced the clipboard text",
            "Replacing the clipboard text",
            "replace the clipboard text",
        ),
        _ => ("other", "Ran a tool", "Running a tool", "run a tool"),
    }
}
fn subject(tool: &str, args: &Value, evidence: &Value) -> Phrase {
    let count = &evidence["count"];
    match tool {
        "read" => {
            let paths = args["paths"].as_array();
            let names = paths
                .map(|p| p.iter().map(path).collect::<Vec<_>>())
                .unwrap_or_default();
            let title = if names.len() == 1 {
                names[0].clone()
            } else {
                plural(names.len(), "path", "paths")
            };
            let mut p = phrase(
                "read",
                format!("Read {title}"),
                format!("Reading {title}"),
                format!("read {title}"),
            );
            if names.len() > 1 {
                p.detail = Some(format!(
                    "{}{}",
                    names.iter().take(2).cloned().collect::<Vec<_>>().join(", "),
                    if names.len() > 2 {
                        format!(" +{} more", names.len() - 2)
                    } else {
                        String::new()
                    }
                ));
            } else if let (Some(start), Some(end)) =
                (args["start_line"].as_i64(), args["end_line"].as_i64())
            {
                p.detail = Some(format!("lines {start}–{end}"));
                if end >= start {
                    p.metric = Some(plural((end - start + 1) as usize, "line", "lines"));
                }
            }
            if let Some(detail) = evidence["detail"].as_str() {
                p.detail = Some(detail.to_string());
                if let Some((a, b)) = detail
                    .strip_prefix("lines ")
                    .and_then(|s| s.split_once('-'))
                    && let (Ok(a), Some(b)) = (
                        a.parse::<i64>(),
                        b.split(' ').next().and_then(|s| s.parse::<i64>().ok()),
                    )
                    && b >= a
                {
                    p.metric = Some(plural((b - a + 1) as usize, "line", "lines"));
                }
            }
            p
        }
        "find" => {
            let q = quote(&args["query"]);
            let mut p = phrase(
                "search",
                format!("Searched {q}"),
                format!("Searching {q}"),
                format!("search {q}"),
            );
            p.detail = if let Some(n) = counter(count) {
                Some(if args["mode"] == "content" {
                    format!("{} in file contents", plural(n, "match", "matches"))
                } else {
                    plural(n, "match", "matches")
                })
            } else if args["mode"] != "content" {
                Some("by name".into())
            } else {
                None
            };
            p
        }
        "apply_patch" => {
            let patch = args["patch"].as_str().unwrap_or("");
            let (kind, act, past, _doing) = if patch.contains("*** Delete File:") {
                ("delete", "delete", "Deleted", "Deleting")
            } else if patch.contains("*** Move to:") {
                ("move", "move", "Moved", "Moving")
            } else if patch.contains("*** Update File:") {
                ("edit", "edit", "Edited", "Editing")
            } else {
                ("create", "create", "Created", "Creating")
            };
            let mut p = phrase(kind, "Applied a patch", "Applying a patch", "apply a patch");
            p.tone = if kind == "delete" { "warn" } else { "good" };
            if let Some(items) = evidence["changes"].as_array().filter(|v| !v.is_empty()) {
                let target = if items.len() == 1 {
                    path(&items[0]["path"])
                } else {
                    plural(items.len(), "file", "files")
                };
                p.past = format!("{past} {target}");
                p.infinitive = format!("{act} {target}");
                // Running captions refer to the patch operation, not a claimed future commit.
                p.doing = "Applying a patch".into();
                let mut plus = 0u64;
                let mut minus = 0u64;
                let mut approx = false;
                for row in items {
                    plus = plus.saturating_add(row["added"].as_u64().unwrap_or(0));
                    minus = minus.saturating_add(row["removed"].as_u64().unwrap_or(0));
                    approx |= row["approximate"] == true;
                }
                if items
                    .iter()
                    .any(|row| row["added"].is_number() || row["removed"].is_number())
                {
                    p.metric = Some(format!("{}+{plus} −{minus}", if approx { "~" } else { "" }));
                }
            }
            p
        }
        "exec_command" => {
            let cmds = args["cmds"].as_array();
            let label = if let Some(cmds) = cmds {
                let clean = cmds.iter().filter_map(Value::as_str).collect::<Vec<_>>();
                if clean.is_empty() {
                    None
                } else {
                    Some(format!("{} commands: {}", clean.len(), clean.join("; ")))
                }
            } else {
                args["cmd"].as_str().and_then(|cmd| {
                    let lines = cmd
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty() && !s.starts_with('#'))
                        .collect::<Vec<_>>();
                    lines.first().map(|first| {
                        format!("{}{}", first, if lines.len() > 1 { " …" } else { "" })
                    })
                })
            };
            let label = label.unwrap_or_else(|| "a command".into());
            let mut p = phrase(
                "run",
                format!("Ran {label}"),
                format!("Running {label}"),
                format!("run {label}"),
            );
            let running = evidence["running"] == true;
            let timedout = evidence["timedOut"] == true;
            let exit = evidence["exitCode"].as_i64();
            if running {
                p.past = format!("Started {label}");
                p.doing = format!("Starting {label}");
                p.infinitive = format!("start {label}");
                p.metric = Some("started".into());
            } else if timedout || exit.is_some_and(|c| c != 0) && evidence["benignExit"] != true {
                p.past = format!("Command failed {label}");
                p.tone = "bad";
                p.metric = Some(if timedout {
                    "✕ timed out".into()
                } else {
                    format!("✕ exit {}", exit.unwrap_or(0))
                });
            } else if let Some(ms) = evidence["durationMs"].as_f64().filter(|v| v.is_finite()) {
                p.tone = "good";
                p.metric = Some(format!("✓ {}", duration(ms)));
            } else if exit == Some(0) {
                p.tone = "good";
            }
            // Running presentation must not infer status from a completed call.
            p
        }
        "write_stdin" => {
            let id = args["session_id"]
                .as_i64()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into());
            let signal = args["signal"].as_str().unwrap_or("");
            let (past, doing, infinitive, tone) = match signal {
                "kill" => ("Stopped", "Stopping", "stop", "warn"),
                "int" => ("Interrupted", "Interrupting", "interrupt", "neutral"),
                _ if args["chars"].as_str().is_some_and(|x| !x.is_empty()) => {
                    ("Wrote to", "Writing to", "write to", "neutral")
                }
                _ => ("Waited on", "Waiting on", "wait on", "neutral"),
            };
            let mut p = phrase(
                "process",
                format!("{past} session {id}"),
                format!("{doing} session {id}"),
                format!("{infinitive} session {id}"),
            );
            p.tone = tone;
            if let Some(exit) = evidence["exitCode"].as_i64()
                && exit != 0
                && evidence["benignExit"] != true
            {
                p.tone = "bad";
                p.metric = Some(format!("✕ exit {exit}"));
            }
            p
        }
        "update_plan" => {
            let plan = args["plan"].as_array();
            let mut p = if plan.is_none() {
                phrase(
                    "session",
                    "Cleared the task plan",
                    "Clearing the task plan",
                    "clear the task plan",
                )
            } else {
                phrase(
                    "session",
                    "Updated the task plan",
                    "Updating the task plan",
                    "update the task plan",
                )
            };
            if let Some(items) = plan {
                let done = items.iter().filter(|v| v["status"] == "completed").count();
                p.detail = Some(format!("{done} / {} completed", items.len()));
            }
            p.special_error = true;
            p
        }
        "computer" => {
            let acts = args["actions"].as_array();
            let n = acts.map_or(0, Vec::len);
            let first = acts
                .and_then(|a| a.first())
                .map(|v| v["type"].as_str().unwrap_or(""))
                .unwrap_or("");
            let (kind, past, doing, infinitive) = match first {
                "read_clipboard" => (
                    "clipboard",
                    "Read the clipboard",
                    "Reading the clipboard",
                    "read the clipboard",
                ),
                "write_clipboard" => (
                    "clipboard",
                    "Replaced the clipboard text",
                    "Replacing the clipboard text",
                    "replace the clipboard text",
                ),
                "click" => ("input", "Clicked", "Clicking", "click"),
                _ => ("clipboard", "Acted", "Acting", "act"),
            };
            let more = n as i64 - 1;
            let extra = format!(" and {more} more");
            let mut p = phrase(
                kind,
                format!("{past}{extra}"),
                format!("{doing}{extra}"),
                format!("{infinitive}{extra}"),
            );
            if n > 0 {
                p.detail = Some(plural(n, "action", "actions"));
            }
            p
        }
        "observe" => {
            let mut p = if args["what"] == "windows" {
                phrase(
                    "screen",
                    "Listed open windows",
                    "Listing open windows",
                    "list open windows",
                )
            } else if args["what"] == "ui" {
                let q = quote(&args["match"]);
                phrase(
                    "screen",
                    format!("Looked for {q}"),
                    format!("Looking for {q}"),
                    format!("look for {q}"),
                )
            } else if let Some(wait) = args["wait_for"].as_str() {
                phrase(
                    "screen",
                    format!("Waited for {wait}"),
                    format!("Waiting for {wait}"),
                    format!("wait for {wait}"),
                )
            } else {
                phrase(
                    "screen",
                    "Looked at the screen",
                    "Looking at the screen",
                    "look at the screen",
                )
            };
            p.detail = counter(count).map(|n| {
                if args["what"] == "windows" {
                    plural(n, "window", "windows")
                } else {
                    plural(n, "match", "matches")
                }
            });
            p
        }
        "session" => {
            let mut p = if args["action"] == "search" {
                let q = quote(&args["query"]);
                phrase(
                    "session",
                    format!("Searched recordings {q}"),
                    format!("Searching recordings {q}"),
                    format!("search recordings {q}"),
                )
            } else if args["action"] == "read" && args["tool_call"].is_string() {
                phrase(
                    "session",
                    "Opened one recorded tool call",
                    "Opening one recorded tool call",
                    "open one recorded tool call",
                )
            } else if args["action"] == "read" && args["cursor"].is_string() {
                phrase(
                    "session",
                    "Continued reading a recorded session",
                    "Running session",
                    "continue reading a recorded session",
                )
            } else {
                phrase(
                    "session",
                    "Listed recent recordings",
                    "Listing recent recordings",
                    "list recent recordings",
                )
            };
            p.detail = detail_count(count, "session", "sessions");
            p
        }
        "agents" => {
            let mut p = match args["action"].as_str().unwrap_or("") {
                "spawn" => {
                    let n = args["workers"].as_array().map_or(0, Vec::len);
                    phrase(
                        "agent",
                        format!("Created {}", plural(n, "worker agent", "worker agents")),
                        format!("Creating {}", plural(n, "worker agent", "worker agents")),
                        format!("create {}", plural(n, "worker agent", "worker agents")),
                    )
                }
                "message" => {
                    if let Some(list) = args["messages"].as_array() {
                        let n = list.len();
                        phrase(
                            "agent",
                            format!("Messaged {}", plural(n, "agent", "agents")),
                            format!("Messaging {}", plural(n, "agent", "agents")),
                            format!("message {}", plural(n, "agent", "agents")),
                        )
                    } else {
                        let to = args["to"].as_str().unwrap_or("agent");
                        phrase(
                            "agent",
                            format!("Messaged {to}"),
                            format!("Messaging {to}"),
                            format!("message {to}"),
                        )
                    }
                }
                "finish" => phrase(
                    "agent",
                    "Reported the finished task",
                    "Reporting the finished task",
                    "report the finished task",
                ),
                _ => phrase(
                    "agent",
                    "Checked agent status",
                    "Checking agent status",
                    "check agent status",
                ),
            };
            if matches!(args["action"].as_str(), Some("spawn" | "finish")) {
                p.tone = "good";
            }
            if args["action"].is_null() {
                p.detail = detail_count(count, "message", "messages");
            }
            p
        }
        _ => {
            let (kind, past, doing, infinitive) = verb(tool);
            if kind == "other" {
                phrase(
                    "other",
                    format!("Ran {tool}"),
                    format!("Running {tool}"),
                    format!("run {tool}"),
                )
            } else {
                phrase(kind, past, doing, infinitive)
            }
        }
    }
}
fn duration(ms: f64) -> String {
    let ms = (ms + 0.5).floor() as i64;
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60000 {
        format!("{:.1}s", (ms as f64 / 1000.0 * 10.0 + 0.5).floor() / 10.0)
    } else {
        format!("{}m", ((ms as f64 / 60000.0) + 0.5).floor() as i64)
    }
}
fn extra_error(detail: &mut Option<String>, result_head: &str) {
    if detail.is_none() && !result_head.is_empty() {
        *detail = Some(result_head.chars().take(160).collect());
    }
}
fn format_summary(mut p: Phrase, tool: &str, outcome: &str, result_head: &str) -> Value {
    match outcome {
        "tool_rejected" => {
            p.tone = "warn";
            p.past = if p.special_error {
                format!("Refused: {}", p.past)
            } else {
                format!("Refused to {}", p.infinitive)
            };
            p.metric = Some("refused".into());
            extra_error(&mut p.detail, result_head);
        }
        "tool_execution_error" => {
            p.tone = "warn";
            p.past = format!("Tool {tool} failed");
            p.metric = Some("✕ failed".into());
            extra_error(&mut p.detail, result_head);
        }
        "tool_internal_error" => {
            p.tone = "bad";
            p.past = if p.special_error {
                format!("Failed: {}", p.past)
            } else {
                format!("Could not {}", p.infinitive)
            };
            p.metric = Some("✕ failed".into());
            extra_error(&mut p.detail, result_head);
        }
        _ => {}
    }
    let mut summary = json!({"kind":p.kind,"tone":p.tone,"title":p.past});
    if let Some(detail) = p.detail {
        summary["detail"] = json!(detail);
    }
    if let Some(metric) = p.metric {
        summary["metric"] = json!(metric);
    }
    summary
}
pub fn completed(
    tool: &str,
    args: &Value,
    evidence: &Value,
    outcome: &str,
    result_head: &str,
) -> Value {
    format_summary(subject(tool, args, evidence), tool, outcome, result_head)
}
pub fn running(tool: &str, args: &Value, evidence: &Value) -> Value {
    let p = subject(tool, args, evidence);
    json!({"title":p.doing,"kind":p.kind})
}
