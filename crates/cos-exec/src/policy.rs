use cos_core::config::{js_len, js_trim};
use serde_json::{Value, json};

#[derive(Debug)]
pub struct Invocation {
    pub args: Vec<String>,
    pub wildcard: bool,
}
pub fn parse(source: &str, shell: &str, rule: bool) -> Result<Invocation, String> {
    let c = source.chars().collect::<Vec<_>>();
    let mut i = 0;
    let mut args = vec![];
    let mut wildcard = false;
    let space = |ch: char| ch == ' ' || ch == '\t';
    while i < c.len() && space(c[i]) {
        i += 1;
    }
    if !rule && shell == "powershell" && c.get(i) == Some(&'&') {
        if !c.get(i + 1).is_some_and(|c| space(*c)) {
            return Err("PowerShell call operator & must be followed by an executable.".into());
        }
        i += 1;
        while i < c.len() && space(c[i]) {
            i += 1;
        }
    }
    let posix = matches!(shell, "bash" | "sh" | "zsh");
    while i < c.len() {
        if matches!(c[i], '\r' | '\n') {
            return Err("Newlines are not supported.".into());
        }
        if rule && c[i] == '*' {
            if args.is_empty() || !js_trim(&c[i + 1..].iter().collect::<String>()).is_empty() {
                return Err(
                    "The wildcard must be a standalone final argument after an executable.".into(),
                );
            }
            wildcard = true;
            break;
        }
        let mut value = String::new();
        let mut has = false;
        while i < c.len() && !space(c[i]) {
            let ch = c[i];
            if matches!(ch, '\r' | '\n') {
                return Err("Newlines are not supported.".into());
            }
            if ch == '"' || ch == '\'' {
                if ch == '\'' && shell == "cmd" {
                    return Err("Single quotes do not quote arguments in cmd.exe.".into());
                }
                if has {
                    return Err("Quoted arguments must be wholly quoted.".into());
                }
                has = true;
                i += 1;
                let mut closed = false;
                while i < c.len() {
                    let q = c[i];
                    if q == ch {
                        closed = true;
                        i += 1;
                        break;
                    }
                    if matches!(q, '\r' | '\n') {
                        return Err("Newlines are not supported.".into());
                    }
                    if ch == '"' {
                        if posix && matches!(q, '$' | '`' | '\\') {
                            return Err(
                                "Expansion and escaping inside double quotes are not supported."
                                    .into(),
                            );
                        }
                        if shell == "powershell" && matches!(q, '$' | '`') {
                            return Err("PowerShell expansion and escaping inside double quotes are not supported.".into());
                        }
                        if shell == "cmd" && matches!(q, '%' | '!') {
                            return Err("cmd.exe variable expansion is not supported.".into());
                        }
                    }
                    value.push(q);
                    i += 1;
                }
                if !closed {
                    return Err(format!(
                        "Unclosed {} quote.",
                        if ch == '"' { "double" } else { "single" }
                    ));
                }
                if i < c.len() && !space(c[i]) {
                    return Err("Quoted arguments must be wholly quoted.".into());
                }
                continue;
            }
            if "|;&<>(){}$`*?[]#".contains(ch)
                || (posix && matches!(ch, '\\' | '~'))
                || (shell == "cmd" && matches!(ch, '^' | '%' | '!'))
                || (shell == "powershell"
                    && (matches!(ch, ',' | '`') || (ch == '@' && value.is_empty())))
            {
                return Err(format!(
                    "Unsupported shell syntax {}.",
                    json!(ch.to_string())
                ));
            }
            has = true;
            value.push(ch);
            i += 1;
        }
        if !has {
            return Err("Expected a literal argument.".into());
        }
        args.push(value);
        while i < c.len() && space(c[i]) {
            i += 1;
        }
    }
    if args.is_empty() {
        return Err("The command is empty.".into());
    }
    if !rule
        && posix
        && regex::Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*=")
            .unwrap()
            .is_match(&args[0])
    {
        return Err("Leading environment assignments are not supported.".into());
    }
    Ok(Invocation { args, wildcard })
}
pub fn validate_rule(rule: &str) -> Result<(), String> {
    if js_len(rule) > 1000 {
        return Err("Rule is longer than 1000 characters.".into());
    }
    parse(js_trim(rule), "neutral", true).map(|_| ())
}
pub fn evaluate(policy: &Value, commands: &[String], shell: &str) -> Value {
    if policy["enabled"] != true {
        return json!({"allowed":true,"args":[]});
    }
    let failure = |i: usize, kind: &str, detail: String| json!({"allowed":false,"commandIndex":i,"kind":kind,"detail":detail});
    let mode = policy["mode"].as_str().unwrap_or("");
    if !matches!(mode, "allow" | "deny") {
        return failure(
            0,
            "invalid-policy",
            "Saved command policy mode is invalid.".into(),
        );
    }
    let Some(saved) = policy["rules"].as_array() else {
        return failure(
            0,
            "invalid-policy",
            "Saved command policy rules are invalid.".into(),
        );
    };
    let mut rules = vec![];
    for (i, rule) in saved.iter().enumerate() {
        let Some(rule) = rule.as_str() else {
            return failure(
                0,
                "invalid-policy",
                format!("Saved rule {} is invalid: Invalid rule.", i + 1),
            );
        };
        if let Err(e) = validate_rule(rule) {
            return failure(
                0,
                "invalid-policy",
                format!("Saved rule {} is invalid: {e}", i + 1),
            );
        }
        rules.push(parse(js_trim(rule), "neutral", true).unwrap());
    }
    let mut parsed = vec![];
    for (i, command) in commands.iter().enumerate() {
        let inv = match parse(command, shell, false) {
            Ok(v) => v,
            Err(e) => return failure(i, "unsupported", e),
        };
        let matched = rules.iter().position(|rule| {
            (if rule.wildcard {
                inv.args.len() >= rule.args.len()
            } else {
                inv.args.len() == rule.args.len()
            }) && rule.args.iter().zip(&inv.args).all(|(a, b)| a == b)
        });
        if mode == "allow" && matched.is_none() {
            return failure(
                i,
                "unmatched",
                "No allowlist rule matched the executable and complete argument list.".into(),
            );
        }
        if mode == "deny"
            && let Some(at) = matched
        {
            return failure(
                i,
                "denied",
                format!(
                    "Denylist rule {} matched the executable and complete argument list.",
                    at + 1
                ),
            );
        }
        parsed.push(inv.args);
    }
    json!({"allowed":true,"args":parsed})
}
pub fn parse_text(text: &str) -> Value {
    let mut rules = vec![];
    let mut issues = vec![];
    for (i, raw) in text.split('\n').enumerate() {
        let rule = js_trim(raw);
        if rule.is_empty() {
            continue;
        }
        let issue = if rules.len() >= 100 {
            Some("At most 100 rules are allowed.".into())
        } else {
            validate_rule(rule).err()
        };
        if let Some(message) = issue {
            issues.push(json!({"line":i+1,"message":message}));
        } else {
            rules.push(rule);
        }
    }
    json!({"rules":rules,"issues":issues})
}
