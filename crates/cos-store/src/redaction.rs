//! Recorder-specific privacy boundary. Transform payloads BEFORE store_text can
//! write inline journal data or overflow assets. Do not use this for tool execution.
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::OnceLock;

const MASK: &str = "[redacted]";

fn normalized(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn sensitive_key(key: &str) -> bool {
    let name = normalized(key);
    [
        "password",
        "passwd",
        "passphrase",
        "secret",
        "token",
        "credential",
        "authorization",
        "cookie",
        "apikey",
        "privatekey",
        "accesskey",
        "sshkey",
        "authheader",
        "bearer",
    ]
    .iter()
    .any(|part| name.contains(part))
}

fn binary_key(key: &str) -> bool {
    matches!(
        normalized(key).as_str(),
        "database64"
            | "base64"
            | "imagebase64"
            | "binary"
            | "filebytes"
            | "blob"
            | "imagedata"
            | "dataurl"
            | "datauri"
    )
}

fn clipboard_action(value: &Value) -> bool {
    value["type"] == "read_clipboard" || value["type"] == "write_clipboard"
}

fn clipboard_read(tool: &str, args: &Value) -> bool {
    tool == "read_clipboard"
        || (tool == "computer"
            && args["actions"].as_array().is_some_and(|actions| {
                actions
                    .iter()
                    .any(|action| action["type"] == "read_clipboard")
            }))
}

fn preserve_env_keys(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .keys()
                .map(|key| (key.clone(), json!(MASK)))
                .collect(),
        ),
        Value::Null => Value::Null,
        _ => json!(MASK),
    }
}

fn unsafe_field(tool: &str, parent: &Value, key: &str) -> bool {
    sensitive_key(key)
        || binary_key(key)
        || (key == "file" && tool == "download_artifact")
        || (key == "data"
            && (parent["type"] == "image"
                || parent["mimeType"]
                    .as_str()
                    .is_some_and(|mime| mime.starts_with("image/"))))
        || (key == "text" && (tool == "write_clipboard" || clipboard_action(parent)))
}

fn sanitise_value(tool: &str, value: &Value) -> Value {
    match value {
        Value::Object(fields) => {
            let mut output = Map::new();
            for (key, field) in fields {
                let clean = if matches!(normalized(key).as_str(), "env" | "environment") {
                    preserve_env_keys(field)
                } else if unsafe_field(tool, value, key) {
                    json!(MASK)
                } else {
                    sanitise_value(tool, field)
                };
                output.insert(key.clone(), clean);
            }
            Value::Object(output)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| sanitise_value(tool, item))
                .collect(),
        ),
        Value::String(text) => json!(sanitise_text(text)),
        _ => value.clone(),
    }
}

/// The recorded arguments must never contain explicit secret-bearing fields,
/// clipboard writes, environment values or encoded binary bodies.
pub fn arguments(tool: &str, args: &Value) -> Value {
    sanitise_value(tool, args)
}

fn patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // Key-value leaks in command lines, output or error messages.
            r#"(?i)\b(?:password|passwd|pwd|token|secret|api[_-]?key|client[_-]?secret|access[_-]?token|refresh[_-]?token)["']?\s*[:=]\s*["']?[^\s,;"']+"#,
            r#"(?i)\b[A-Za-z][A-Za-z0-9_]*(?:TOKEN|SECRET|PASSWORD|PASSWD|API_KEY|ACCESS_KEY|PRIVATE_KEY|CLIENT_SECRET)\s*[:=]\s*["']?[^\s,;"']+"#,
            r"(?i)\b(?:authorization|proxy-authorization)\s*[:=]\s*(?:bearer|basic)?\s*\S+",
            r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
            r"(?-u:\b)ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}(?-u:\b)",
            r"(?i)data:[^\s;,]+;base64,[A-Za-z0-9+/=_-]+",
            r"(?-u:\b)[A-Za-z0-9+/]{96,}={0,2}(?-u:\b)",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("static redaction regex"))
        .collect()
    })
}

fn sanitise_text(text: &str) -> String {
    let mut clean = cos_core::logging::redact_credentials(text);
    for pattern in patterns() {
        clean = pattern.replace_all(&clean, MASK).into_owned();
    }
    clean
}

fn secret_values(
    tool: &str,
    value: &Value,
    parent: Option<&Value>,
    key: &str,
    out: &mut Vec<String>,
    overflow: &mut bool,
) {
    let protected = sensitive_key(key)
        || matches!(normalized(key).as_str(), "env" | "environment")
        || binary_key(key)
        || parent.is_some_and(|owner| unsafe_field(tool, owner, key));
    if protected {
        match value {
            Value::String(text) => {
                if !text.is_empty() {
                    if out.len() >= 256 || text.len() > 4096 {
                        *overflow = true;
                    } else {
                        out.push(text.clone());
                    }
                }
            }
            Value::Object(fields) => {
                for field in fields.values() {
                    secret_values(tool, field, Some(value), key, out, overflow);
                }
            }
            Value::Array(items) => {
                for item in items {
                    secret_values(tool, item, Some(value), key, out, overflow);
                }
            }
            _ => {}
        }
        return;
    }
    match value {
        Value::Object(fields) => {
            for (child, field) in fields {
                secret_values(tool, field, Some(value), child, out, overflow);
            }
        }
        Value::Array(items) => {
            for item in items {
                secret_values(tool, item, Some(value), "", out, overflow);
            }
        }
        _ => {}
    }
}

/// The raw result is never persisted if it can contain a clipboard read or
/// we cannot safely subtract the known sensitive inputs. Call before store_text.
pub fn result(tool: &str, args: &Value, text: &str) -> String {
    if clipboard_read(tool, args) || (tool == "computer" && text.contains("Clipboard read ")) {
        return "<clipboard result not stored>".into();
    }
    let mut values = Vec::new();
    let mut overflow = false;
    secret_values(tool, args, None, "", &mut values, &mut overflow);
    if overflow {
        return "<result not stored: sensitive input too large>".into();
    }
    // Longer substrings first, so redaction of one value cannot expose another.
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    let mut clean = text.to_owned();
    for secret in values {
        clean = clean.replace(&secret, MASK);
    }
    sanitise_text(&clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_args_and_results_are_scrubbed_without_touching_public_fields() {
        let args = json!({
            "path": "/safe/file.txt",
            "env": {"HOME": "/secret/home", "API_TOKEN": "env-value-123"},
            "nested": {"password": "private-value", "dataBase64": "U0VDUkVU"},
            "actions": [{"type": "write_clipboard", "text": "clipboard-secret"}],
            "cmd": "printf 'token=abc123'"
        });
        let output = arguments("computer", &args).to_string();
        for needle in [
            "private-value",
            "env-value-123",
            "clipboard-secret",
            "U0VDUkVU",
            "token=abc123",
        ] {
            assert!(!output.contains(needle), "{needle} survived");
        }
        assert!(output.contains("/safe/file.txt"));
        let result = result(
            "computer",
            &args,
            "env-value-123 private-value clipboard-secret public",
        );
        assert!(!result.contains("env-value-123"));
        assert!(!result.contains("private-value"));
        assert!(!result.contains("clipboard-secret"));
        assert!(result.contains("public"));
    }

    #[test]
    fn json_credentials_are_not_written_to_results() {
        let clean = result(
            "read",
            &json!({}),
            r#"{"token":"RESULT_SECRET_321", "status":"ok"}"#,
        );
        assert!(!clean.contains("RESULT_SECRET_321"));
        assert!(clean.contains("ok"));
        let env = result(
            "exec_command",
            &json!({}),
            "OPENAI_API_KEY=ENV_RESULT_SECRET",
        );
        assert!(!env.contains("ENV_RESULT_SECRET"));
    }

    #[test]
    fn clipboard_reads_fail_closed() {
        assert_eq!(
            result("read_clipboard", &json!({}), "anything"),
            "<clipboard result not stored>"
        );
        assert_eq!(
            result(
                "computer",
                &json!({"actions":[{"type":"read_clipboard"}]}),
                "anything"
            ),
            "<clipboard result not stored>"
        );
    }
}
