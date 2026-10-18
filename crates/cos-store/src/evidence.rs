//! Trusted, bounded runtime facts used by later activity captions.
//! Never parse prose to infer whether a command succeeded or which file changed.
use serde_json::{Value, json};

fn process_evidence(structured: &Value) -> (Value, Value, Value, Value) {
    let exit = structured["exit_code"]
        .as_i64()
        .map_or(Value::Null, |value| json!(value));
    let session = structured["session_id"]
        .as_i64()
        .or_else(|| structured["completed_session_id"].as_i64());
    let running = if structured["session_id"].as_i64().is_some() {
        json!(true)
    } else if structured["exit_code"].as_i64().is_some() {
        json!(false)
    } else {
        Value::Null
    };
    let duration = structured["wall_time_seconds"]
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0 && *value < 86_400.0)
        .map_or(Value::Null, |value| json!((value * 1000.0).round() as u64));
    (
        exit,
        running,
        session.map_or(Value::Null, |value| json!(value)),
        duration,
    )
}

fn changes(value: &Value) -> Value {
    let Some(items) = value.as_array() else {
        return json!([]);
    };
    let mut rows = Vec::new();
    for change in items.iter().take(128) {
        let Some(path) = change["path"].as_str().filter(|path| path.len() <= 4096) else {
            continue;
        };
        let Some(kind) = change["kind"]
            .as_str()
            .filter(|kind| matches!(*kind, "add" | "update" | "delete" | "move"))
        else {
            continue;
        };
        rows.push(json!({
            "path":path,
            "kind":kind,
            "added":change["added"].as_u64(),
            "removed":change["removed"].as_u64(),
            "approximate":change["approximate"] == true,
        }));
    }
    json!(rows)
}

/// Read only trusted machine-generated fields. `private` is removed from the
/// public MCP result before it is returned to the caller or persisted.
pub fn from_runtime(tool: &str, response: &Value, private: &Value) -> Value {
    let structured = &response["structuredContent"];
    let failed = response["isError"] == true;
    let (exit_code, running, process_session_id, duration_ms) =
        if !failed && matches!(tool, "exec_command" | "write_stdin") {
            process_evidence(structured)
        } else {
            (Value::Null, Value::Null, Value::Null, Value::Null)
        };
    let count = if !failed && tool == "find" {
        private["count"].as_u64().map_or(Value::Null, |count| json!(count))
    } else {
        Value::Null
    };
    let report = if failed {
        &Value::Null
    } else if tool == "apply_patch" {
        &structured["changes"]
    } else if tool == "exec_command" {
        &private["changes"]
    } else {
        &Value::Null
    };
    json!({
        "count":count,
        "changes":changes(report),
        "exitCode":exit_code,
        "running":running,
        "processSessionId":process_session_id,
        "durationMs":duration_ms,
        "timedOut":structured["timed_out"] == true,
        "failed":failed,
    })
}

/// Path evidence is derived from a committed patch report, never a proposed
/// patch body. Still mask credentials before the journal/asset boundary.
pub fn for_recording(tool: &str, args: &Value, mut evidence: Value) -> Value {
    if let Some(changes) = evidence["changes"].as_array_mut() {
        for item in changes {
            if let Some(path) = item["path"].as_str() {
                item["path"] = json!(crate::redaction::result(tool, args, path));
            }
        }
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_then_completed_process_only_from_structured_fields() {
        let running = from_runtime(
            "exec_command",
            &json!({"content":[{"type":"text","text":"exit code 99"}],"structuredContent":{"session_id":42,"wall_time_seconds":0.35}}),
            &Value::Null,
        );
        assert_eq!(running["running"], true);
        assert_eq!(running["exitCode"], Value::Null);
        assert_eq!(running["processSessionId"], 42);
        assert_eq!(running["durationMs"], 350);
        let completed = from_runtime(
            "write_stdin",
            &json!({"structuredContent":{"completed_session_id":42,"exit_code":17,"wall_time_seconds":0.9}}),
            &Value::Null,
        );
        assert_eq!(completed["running"], false);
        assert_eq!(completed["exitCode"], 17);
        assert_eq!(completed["processSessionId"], 42);
        let ambiguous = from_runtime("exec_command", &json!({"content":[{"type":"text","text":"Completed successfully"}]}), &Value::Null);
        assert_eq!(ambiguous["running"], Value::Null);
        assert_eq!(ambiguous["exitCode"], Value::Null);
    }

    #[test]
    fn bounded_trusted_find_and_patch_facts() {
        let good = json!({"content":[{"type":"text","text":"success"}]});
        assert_eq!(from_runtime("find", &good, &json!({"count":3}))["count"], 3);
        assert!(from_runtime("find", &good, &Value::Null)["count"].is_null());
        assert!(from_runtime("find", &json!({"isError":true}), &json!({"count":3}))["count"].is_null());
        let patch = from_runtime("apply_patch",
            &json!({"structuredContent":{"changes":[{"path":"/p/a.txt","kind":"add","added":2,"removed":0},{"path":"/p/b.txt","kind":"update","added":1,"removed":1}]}}),
            &Value::Null);
        assert_eq!(patch["changes"].as_array().unwrap().len(), 2);
        let failed = from_runtime("apply_patch",
            &json!({"isError":true,"structuredContent":{"changes":[{"path":"/p/a.txt","kind":"add"}]}}),
            &Value::Null);
        assert_eq!(failed["changes"], json!([]));
    }
}
