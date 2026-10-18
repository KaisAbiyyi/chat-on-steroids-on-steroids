//! The source deliberately applies the same tiny Bash query to every outer shell.
use super::{Parsed, parse_typed};
use cos_core::config::js_len;
use serde::Serialize;
use std::sync::OnceLock;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

const IMPLICIT: &str =
    "patch detected without explicit call to apply_patch. Rerun as [\"apply_patch\", \"<patch>\"]";
const QUERY: &str =
    include_str!("../../../../fixtures/upstream/phase1-6-closure/apply-patch-query.scm");
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Invocation {
    Body { args: Parsed },
    PatchParseError { error: String },
    CorrectnessError { error: String },
    NotApplyPatch,
}
fn admission(argv: &[String]) -> Result<(), String> {
    let units = argv
        .iter()
        .fold(0usize, |n, arg| n.saturating_add(js_len(arg)));
    if units > 4 * 1024 * 1024 {
        return Err("apply_patch invocation exceeds 4 MiB character limit".into());
    }
    let bytes = argv
        .iter()
        .fold(0usize, |n, arg| n.saturating_add(arg.len()));
    let breaks = argv.iter().fold(1usize, |n, arg| {
        n.saturating_add(arg.bytes().filter(|b| matches!(b, b'\r' | b'\n')).count())
    });
    crate::patch_budget::check_source_counts(bytes, breaks).map_err(|e| e.to_string())
}
fn shell_name<'a>(shell: &'a str, cwd: &str) -> &'a str {
    let bytes = cwd.as_bytes();
    let windows = cwd.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'));
    let basename = if windows {
        let path = shell.trim_end_matches(['/', '\\']);
        let path =
            if path.as_bytes().get(1) == Some(&b':') && path.as_bytes()[0].is_ascii_alphabetic() {
                &path[2..]
            } else {
                path
            };
        path.rsplit(['/', '\\']).next().unwrap_or("")
    } else {
        shell.trim_end_matches('/').rsplit('/').next().unwrap_or("")
    };
    match basename.rfind('.') {
        Some(dot) if dot > 0 => &basename[..dot],
        _ => basename,
    }
}
fn shell_script<'a>(argv: &'a [String], cwd: &str) -> Option<&'a str> {
    let (shell, flag, script) = match argv {
        [shell, flag, script] => (shell, flag, script),
        [shell, skip, flag, script]
            if matches!(
                shell_name(shell, cwd).to_lowercase().as_str(),
                "pwsh" | "powershell"
            ) && skip.eq_ignore_ascii_case("-noprofile") =>
        {
            (shell, flag, script)
        }
        _ => return None,
    };
    match shell_name(shell, cwd).to_lowercase().as_str() {
        "bash" | "zsh" | "sh" if matches!(flag.as_str(), "-c" | "-lc") => Some(script),
        "pwsh" | "powershell" if flag.eq_ignore_ascii_case("-command") => Some(script),
        "cmd" if flag.eq_ignore_ascii_case("/c") => Some(script),
        _ => None,
    }
}
fn extract(script: &str) -> Option<(String, Option<String>)> {
    static COMPILED: OnceLock<Option<Query>> = OnceLock::new();
    let language = tree_sitter_bash::LANGUAGE.into();
    let query = COMPILED
        .get_or_init(|| Query::new(&language, QUERY).ok())
        .as_ref()?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(script, None)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), script.as_bytes());
    while let Some(found) = matches.next() {
        let mut body = None;
        let mut workdir = None;
        for capture in found.captures {
            let text = capture.node.utf8_text(script.as_bytes()).ok()?;
            match query.capture_names()[capture.index as usize] {
                "heredoc" => body = Some(text.trim_end_matches('\n').to_owned()),
                "cd_path" => workdir = Some(text.to_owned()),
                "cd_raw_string" => {
                    workdir = Some(
                        text.strip_prefix('\'')
                            .and_then(|s| s.strip_suffix('\''))
                            .unwrap_or(text)
                            .to_owned(),
                    )
                }
                _ => {}
            }
        }
        if let Some(body) = body {
            return Some((body, workdir));
        }
    }
    None
}
pub fn maybe_parse(argv: &[String], cwd: &str) -> Invocation {
    if let Err(error) = admission(argv) {
        return Invocation::PatchParseError { error };
    }
    if let [command, body] = argv
        && matches!(command.as_str(), "apply_patch" | "applypatch")
    {
        return match parse_typed(body) {
            Ok(args) => Invocation::Body { args },
            Err(error) => Invocation::PatchParseError { error },
        };
    }
    let Some((body, workdir)) = shell_script(argv, cwd).and_then(extract) else {
        return Invocation::NotApplyPatch;
    };
    match parse_typed(&body) {
        Ok(mut args) => {
            args.workdir = workdir;
            Invocation::Body { args }
        }
        Err(error) => Invocation::PatchParseError { error },
    }
}
pub fn for_exec(argv: &[String], cwd: &str) -> Invocation {
    if let Err(error) = admission(argv) {
        return Invocation::CorrectnessError { error };
    }
    if matches!(argv,[body] if parse_typed(body).is_ok())
        || shell_script(argv, cwd).is_some_and(|body| parse_typed(body).is_ok())
    {
        return Invocation::CorrectnessError {
            error: IMPLICIT.into(),
        };
    }
    match maybe_parse(argv, cwd) {
        Invocation::PatchParseError { error } => Invocation::CorrectnessError { error },
        other => other,
    }
}
