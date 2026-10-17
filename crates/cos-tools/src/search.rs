use crate::{
    files::{Files, glob_regex, locate},
    text,
};
use cos_core::{config::Root, persistence::invalid};
use serde_json::{Value, json};
use std::{
    io,
    time::{Duration, Instant},
};
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "dist",
    "build",
    "out",
    "target",
    ".next",
    ".nuxt",
    ".gradle",
    ".gradle-user",
    ".idea",
    ".vs",
    ".vscode",
    "__pycache__",
    ".venv",
    "venv",
    "coverage",
    ".cache",
    ".turbo",
    ".go-build-cache",
    ".go-module-cache",
    ".go-tools",
    ".gopath",
    ".tmp",
    ".tmp-go-cache",
    ".tmp-go-path",
    ".android",
    ".bun",
    ".claude",
    ".claude-*",
    ".codex",
    ".cursor",
    ".gemini",
    ".npm",
    ".pnpm-store",
    ".yarn",
    "release",
    "release-*",
    "appdata",
];
pub struct Options<'a> {
    pub query: &'a str,
    pub content: bool,
    pub glob: Option<&'a str>,
    pub exclude: Option<&'a [String]>,
    pub limit: usize,
    pub case_sensitive: bool,
    pub regex: bool,
}
impl<'a> Options<'a> {
    pub fn new(query: &'a str) -> Self {
        Self {
            query,
            content: false,
            glob: None,
            exclude: None,
            limit: 50,
            case_sensitive: false,
            regex: false,
        }
    }
}
pub fn find_options(
    files: &Files,
    roots: &[Root],
    base: &str,
    options: &Options<'_>,
) -> io::Result<Value> {
    find_checked(files, roots, base, options, &|| Ok(()))
}
pub fn find_checked(
    files: &Files,
    roots: &[Root],
    base: &str,
    options: &Options<'_>,
    authorize: &dyn Fn() -> io::Result<()>,
) -> io::Result<Value> {
    authorize()?;
    let Options {
        query,
        content,
        glob,
        exclude,
        limit,
        case_sensitive,
        regex,
    } = *options;
    if limit == 0 || limit > 500 {
        return Err(invalid("limit must be between 1 and 500"));
    }
    if regex && !content {
        return Err(invalid("regex=true is only valid with mode=content"));
    }
    let excludes = exclude
        .map(|v| v.to_vec())
        .unwrap_or_else(|| DEFAULT_EXCLUDES.iter().map(|s| (*s).into()).collect());
    let deadline = Instant::now() + Duration::from_secs(10);
    let location = locate(roots, base, None, false)?;
    let walked = if location.root.metadata(&location.relative)?.is_file() {
        crate::files::WalkResult {
            paths: vec![location.virtual_path],
            truncated: false,
        }
    } else {
        files.walk_checked(roots, base, &excludes, 40000, deadline, authorize)?
    };
    let pattern = glob.map(glob_regex).transpose()?;
    let mut matches = vec![];
    let mut skipped = 0;
    let mut scanned = 0;
    let mut truncated = walked.truncated;
    let expression = if regex {
        Some(
            regex::RegexBuilder::new(query)
                .case_insensitive(!case_sensitive)
                .size_limit(1024 * 1024)
                .build()
                .map_err(|e| invalid(format!("Invalid regex: {e}")))?,
        )
    } else {
        None
    };
    let query = if case_sensitive {
        query.to_owned()
    } else {
        query.to_lowercase()
    };
    let matching = |text: &str| {
        if let Some(regex) = &expression {
            regex.is_match(text)
        } else if case_sensitive {
            text.contains(&query)
        } else {
            text.to_lowercase().contains(&query)
        }
    };
    for path in walked.paths {
        authorize()?;
        if matches.len() >= limit.min(1000) || Instant::now() > deadline {
            truncated = true;
            break;
        }
        let relative = path
            .strip_prefix(&format!("{}/", base.trim_end_matches('/')))
            .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(&path));
        if pattern.as_ref().is_some_and(|g| {
            !g.is_match(if glob.is_some_and(|s| s.starts_with('/')) {
                &path
            } else {
                relative
            })
        }) {
            continue;
        }
        scanned += 1;
        if content {
            let location = locate(roots, &path, None, false)?;
            let raw = match location.open(2 * 1024 * 1024) {
                Ok(bytes) => bytes,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let (text, _) = match text::decode(&raw) {
                Ok(t) => t,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            for (line, text) in text.lines().enumerate() {
                if line % 256 == 0 {
                    authorize()?;
                    if Instant::now() > deadline {
                        truncated = true;
                        break;
                    }
                }
                if matching(text) {
                    matches.push(json!({"path":path,"line":line+1,"text":text.trim().chars().take(300).collect::<String>()}));
                    if matches.len() >= limit.min(1000) {
                        truncated = true;
                        break;
                    }
                }
            }
        } else if matching(&path) {
            matches.push(json!({"path":path}));
        }
    }
    if limit == 0 {
        return Err(invalid("limit must be positive"));
    }
    authorize()?;
    Ok(
        json!({"matches":matches,"truncated":truncated,"scanned":scanned,"skipped":skipped,"content_file_limit":2*1024*1024}),
    )
}
