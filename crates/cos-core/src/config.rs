//! Executes the frozen upstream validation contract in Rust, including Zod's missing/default semantics.
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{collections::HashSet, sync::OnceLock};

pub const CAPABILITIES: [&str; 13] = [
    "browse",
    "search",
    "read",
    "metadata",
    "create",
    "edit",
    "move",
    "deleteFile",
    "command",
    "screen",
    "control",
    "clipboardRead",
    "clipboardWrite",
];
pub const WRITE_CAPABILITIES: [&str; 7] = [
    "create",
    "edit",
    "move",
    "deleteFile",
    "command",
    "control",
    "clipboardWrite",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Root {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub path: String,
}
type Parsed = Result<Option<Value>, Vec<ValidationIssue>>;

fn rules() -> &'static Value {
    static RULES: OnceLock<Value> = OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(include_str!("../../../fixtures/upstream/config-rules.json"))
            .expect("pinned validation rules")
    })
}
fn metadata() -> &'static Value {
    static META: OnceLock<Value> = OnceLock::new();
    META.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/upstream/config-migrations.json"
        ))
        .expect("pinned migrations")
    })
}
fn fail(path: &str) -> Parsed {
    Err(vec![ValidationIssue { path: path.into() }])
}
fn constant(value: &Value) -> Option<Value> {
    if value.get("$undefined") == Some(&Value::Bool(true)) {
        None
    } else {
        Some(value.clone())
    }
}
pub fn js_len(value: &str) -> usize {
    value.encode_utf16().count()
}
pub fn js_trim(value: &str) -> &str {
    value.trim_matches(|c: char| matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
}

pub fn parse(value: Value) -> Result<Value, Vec<ValidationIssue>> {
    use sha2::{Digest, Sha256};
    // Regeneration must be paired with a reviewed port. An unknown transform under
    // a Zod catch must never silently become an accepted fallback configuration.
    static CONTRACT_OK: OnceLock<bool> = OnceLock::new();
    if !CONTRACT_OK.get_or_init(|| {
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../fixtures/upstream/config-rules.json"
            ))
        ) == "a38779b566636af4e5129f8bf7be746182f65c561edd31c884e7e0b2997e9fa9"
    }) {
        return Err(vec![ValidationIssue {
            path: "$unsupported_config_contract".into(),
        }]);
    }
    evaluate(rules(), Some(value), "").map(|v| v.expect("root configuration is required"))
}

fn evaluate(rule: &Value, input: Option<Value>, path: &str) -> Parsed {
    let kind = rule["type"].as_str().unwrap_or("");
    match kind {
        "optional" => {
            return if input.is_none() {
                Ok(None)
            } else {
                evaluate(&rule["inner"], input, path)
            };
        }
        "default" => {
            return if input.is_none() {
                Ok(constant(&rule["value"]))
            } else {
                evaluate(&rule["inner"], input, path)
            };
        }
        "catch" => {
            return evaluate(&rule["inner"], input, path).or_else(|_| Ok(constant(&rule["value"])));
        }
        "pipe" => {
            return evaluate(&rule["input"], input, path)
                .and_then(|v| evaluate(&rule["output"], v, path));
        }
        "transform" => return transform(rule["source"].as_str().unwrap_or(""), input, path),
        _ => {}
    }
    let Some(mut value) = input else {
        return fail(path);
    };
    match kind {
        "object" => {
            let Some(original) = value.as_object() else {
                return fail(path);
            };
            let properties = rule["properties"].as_object().expect("object contract");
            let mut output = Map::new();
            let mut issues = Vec::new();
            if rule["strict"] == true && original.keys().any(|key| !properties.contains_key(key)) {
                issues.push(ValidationIssue { path: path.into() });
            }
            for (name, property) in properties {
                let next_path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };
                match evaluate(property, original.get(name).cloned(), &next_path) {
                    Ok(Some(result)) => {
                        output.insert(name.clone(), result);
                    }
                    Ok(None) => {}
                    Err(mut errors) => issues.append(&mut errors),
                }
            }
            if !issues.is_empty() {
                return Err(issues);
            }
            value = Value::Object(output);
        }
        "array" => {
            let Some(items) = value.as_array() else {
                return fail(path);
            };
            let mut output = Vec::new();
            let mut issues = Vec::new();
            for (index, item) in items.iter().enumerate() {
                match evaluate(
                    &rule["element"],
                    Some(item.clone()),
                    &format!("{path}.{index}"),
                ) {
                    Ok(result) => output.push(result.unwrap_or(Value::Null)),
                    Err(mut errors) => issues.append(&mut errors),
                }
            }
            if !issues.is_empty() {
                return Err(issues);
            }
            value = Value::Array(output);
        }
        "boolean" if !value.is_boolean() => return fail(path),
        "string" if !value.is_string() => return fail(path),
        "number" if !value.is_number() => return fail(path),
        "enum" | "literal" => {
            if !rule["values"]
                .as_array()
                .expect("enum values")
                .contains(&value)
            {
                return fail(path);
            }
        }
        "union" => {
            for option in rule["options"].as_array().expect("union options") {
                if let Ok(result) = evaluate(option, Some(value.clone()), path) {
                    return Ok(result);
                }
            }
            return fail(path);
        }
        "boolean" | "string" | "number" => {}
        _ => return fail(path),
    }
    for check in rule["checks"].as_array().into_iter().flatten() {
        let valid = match check["check"].as_str().unwrap_or("") {
            "overwrite" => {
                if check["source"]
                    .as_str()
                    .is_some_and(|s| s.contains("input.trim()"))
                {
                    value = Value::String(js_trim(value.as_str().unwrap_or("")).into());
                    true
                } else {
                    false
                }
            }
            // The pinned Zod 4.6.5 counts Unicode code points here; explicit JS slices/tokens still count UTF-16 units.
            "min_length" => value
                .as_str()
                .map(|s| s.chars().count())
                .or_else(|| value.as_array().map(Vec::len))
                .is_some_and(|n| n >= check["minimum"].as_u64().unwrap_or(0) as usize),
            "max_length" => value
                .as_str()
                .map(|s| s.chars().count())
                .or_else(|| value.as_array().map(Vec::len))
                .is_some_and(|n| n <= check["maximum"].as_u64().unwrap_or(0) as usize),
            "min_size" => value
                .as_array()
                .is_some_and(|v| v.len() >= check["minimum"].as_u64().unwrap_or(0) as usize),
            "max_size" => value
                .as_array()
                .is_some_and(|v| v.len() <= check["maximum"].as_u64().unwrap_or(0) as usize),
            "greater_than" => value.as_f64().is_some_and(|v| {
                if check["inclusive"] == true {
                    v >= check["value"].as_f64().unwrap()
                } else {
                    v > check["value"].as_f64().unwrap()
                }
            }),
            "less_than" => value.as_f64().is_some_and(|v| {
                if check["inclusive"] == true {
                    v <= check["value"].as_f64().unwrap()
                } else {
                    v < check["value"].as_f64().unwrap()
                }
            }),
            "number_format" => value.as_f64().is_some_and(|v| {
                v.is_finite() && v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_991.0
            }),
            "string_format" => {
                let pattern = check["pattern"].as_str().unwrap_or("").replace("\\/", "/");
                Regex::new(&pattern)
                    .ok()
                    .is_some_and(|regex| value.as_str().is_some_and(|v| regex.is_match(v)))
            }
            "custom" if path == "setupProfiles" => {
                let items = value.as_array().unwrap();
                let mut ids = HashSet::new();
                items.iter().all(|v| ids.insert(v["id"].as_str()))
            }
            "custom" if path == "sessions.storageLimitGiB" => {
                [2, 5, 10, 20, 50].contains(&value.as_i64().unwrap_or(-1))
            }
            "custom" if path.starts_with("commandAllowlist.rules.") => {
                value.as_str().is_some_and(valid_command_rule)
            }
            _ => false,
        };
        if !valid {
            return fail(path);
        }
    }
    Ok(Some(value))
}

fn transform(source: &str, input: Option<Value>, path: &str) -> Parsed {
    let Some(mut value) = input else {
        return fail(path);
    };
    if source.contains("function uniqueStoredRoots") {
        let Some(roots) = value.as_array_mut() else {
            return fail(path);
        };
        let mut used = HashSet::new();
        for root in roots {
            let wanted = root["name"].as_str().unwrap_or("").to_string();
            let base = if wanted == "skills" {
                format!("{wanted}-folder")
            } else {
                wanted
            };
            let mut candidate: String = base.chars().take(32).collect();
            let mut suffix = 2;
            while candidate == "skills" || used.contains(&candidate) {
                let tail = format!("-{suffix}");
                candidate = format!(
                    "{}{}",
                    base.chars().take(32 - tail.len()).collect::<String>(),
                    tail
                );
                suffix += 1;
            }
            root["name"] = Value::String(candidate.clone());
            used.insert(candidate);
        }
    } else if source.contains("function migrateCapabilities") {
        if let Some(object) = value.as_object_mut() {
            if object.get("powershell") == Some(&Value::Bool(true)) {
                object.insert("command".into(), Value::Bool(true));
            }
            object.remove("powershell");
            object.remove("deleteFolder");
        }
    } else if source.contains("DEFAULT_CAPABILITIES") {
        let Some(caps) = value.as_object() else {
            return fail(path);
        };
        let mut combined = metadata()["DEFAULT_CAPABILITIES"]
            .as_object()
            .unwrap()
            .clone();
        combined.extend(caps.clone());
        value = Value::Object(combined);
    } else if source.contains("record: true") {
        value["record"] = Value::Bool(true);
        value["retainDays"] = json!(0);
    } else if source.contains("DEFAULT_COMPACTION.handoffPrompt") || source.contains("DEFAULT_GOAL")
    {
        let Some(text) = value.as_str() else {
            return fail(path);
        };
        let trimmed = js_trim(text);
        let key = path.rsplit('.').next().unwrap_or(path);
        value = if trimmed.is_empty() {
            let defaults = crate::fresh_config("win32");
            if path.starts_with("compaction.") {
                defaults["compaction"][key].clone()
            } else {
                defaults["goal"][key].clone()
            }
        } else {
            Value::String(trimmed.into())
        };
    } else if source.contains("MAX_MCP_INSTRUCTIONS_CHARS") {
        let Some(text) = value.as_str() else {
            return fail(path);
        };
        let units: Vec<_> = text.encode_utf16().take(4000).collect();
        let Ok(sliced) = String::from_utf16(&units) else {
            return fail(path);
        };
        value = Value::String(js_trim(&sliced).into());
    } else if source.contains("allowActions: value.enabled && value.allowActions") {
        value["allowActions"] = json!(value["enabled"] == true && value["allowActions"] == true);
    } else if source != "(size) => size" {
        return fail(path);
    }
    Ok(Some(value))
}

pub fn migrate_loaded(mut value: Value) -> Value {
    let defaults = crate::fresh_config("win32");
    if [(180000, 200000), (300000, 400000)].iter().any(|(a, l)| {
        value["sessions"]["advisoryTokens"] == *a && value["sessions"]["limitTokens"] == *l
    }) {
        value["sessions"]["advisoryTokens"] = defaults["sessions"]["advisoryTokens"].clone();
        value["sessions"]["limitTokens"] = defaults["sessions"]["limitTokens"].clone();
    }
    if value["compaction"]["autoTokens"] == 300000 {
        value["compaction"]["auto"] = json!(true);
        value["compaction"]["autoTokens"] = defaults["compaction"]["autoTokens"].clone();
    }
    for (field, list) in [
        ("prompt", "SUPERSEDED_GOAL_SYSTEM_PROMPTS"),
        (
            "objectivePrompt",
            "SUPERSEDED_GOAL_OBJECTIVE_SYSTEM_PROMPTS",
        ),
        ("loopPrompt", "SUPERSEDED_GOAL_LOOP_SYSTEM_PROMPTS"),
        ("helperModel", "SUPERSEDED_HELPER_CHAT_MODELS"),
    ] {
        if metadata()[list]
            .as_array()
            .is_some_and(|items| items.contains(&value["goal"][field]))
        {
            value["goal"][field] = defaults["goal"][field].clone();
        }
    }
    value
}

pub fn recovery_config() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/config-recovery.json"
    ))
    .unwrap()
}

pub fn effective_capabilities(config: &Value, platform: &str) -> Map<String, Value> {
    #[cfg(target_os = "macos")]
    if platform == "darwin" {
        let release = darwin_release();
        return effective_capabilities_for_release(config, platform, Some(&release));
    }
    effective_capabilities_for_release(config, platform, None)
}
#[cfg(target_os = "macos")]
fn darwin_release() -> String {
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const std::ffi::c_char,
            old: *mut std::ffi::c_void,
            length: *mut usize,
            new: *mut std::ffi::c_void,
            new_length: usize,
        ) -> std::ffi::c_int;
    }
    let mut buffer = [0_u8; 128];
    let mut length = buffer.len();
    let result = unsafe {
        sysctlbyname(
            c"kern.osrelease".as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || length > buffer.len() {
        return String::new();
    }
    String::from_utf8(
        buffer[..length]
            .iter()
            .copied()
            .take_while(|b| *b != 0)
            .collect(),
    )
    .unwrap_or_default()
}
pub fn effective_capabilities_for_release(
    config: &Value,
    platform: &str,
    release: Option<&str>,
) -> Map<String, Value> {
    let mut caps = config["capabilities"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let supported = match platform {
        "win32" => true,
        "darwin" => release.is_none_or(|release| {
            let mut parts = release.split('.').map(|part| {
                part.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<u32>()
                    .unwrap_or(0)
            });
            let major = parts.next().unwrap_or(0);
            let minor = parts.next().unwrap_or(0);
            major > 21 || major == 21 && minor >= 4
        }),
        _ => false,
    };
    if !supported {
        for key in ["clipboardRead", "clipboardWrite"] {
            caps.insert(key.into(), json!(false));
        }
    }
    if config["readOnly"] == true {
        for key in WRITE_CAPABILITIES {
            caps.insert(key.into(), json!(false));
        }
    }
    caps
}

fn valid_command_rule(rule: &str) -> bool {
    let chars: Vec<char> = js_trim(rule).chars().collect();
    let mut index = 0;
    let mut arguments = 0;
    while index < chars.len() {
        while index < chars.len() && matches!(chars[index], ' ' | '\t') {
            index += 1;
        }
        if index == chars.len() {
            break;
        }
        if chars[index] == '*' {
            return arguments > 0 && chars[index + 1..].iter().all(|c| matches!(c, ' ' | '\t'));
        }
        if matches!(chars[index], '"' | '\'') {
            let quote = chars[index];
            index += 1;
            while index < chars.len() && chars[index] != quote {
                if matches!(chars[index], '\r' | '\n') {
                    return false;
                }
                index += 1;
            }
            if index == chars.len() {
                return false;
            }
            index += 1;
            if index < chars.len() && !matches!(chars[index], ' ' | '\t') {
                return false;
            }
        } else {
            while index < chars.len() && !matches!(chars[index], ' ' | '\t') {
                if "|;&<>(){}\r\n$`*?[]#\"'".contains(chars[index]) {
                    return false;
                }
                index += 1;
            }
        }
        arguments += 1;
    }
    arguments > 0
}
