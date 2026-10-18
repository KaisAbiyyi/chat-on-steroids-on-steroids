use crate::files::Files;
use cos_core::config::Root;
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
pub mod engine;
mod matcher;
pub mod native;
mod scheduling;
pub mod server;
pub use matcher::IncludeMatcher;
#[derive(Clone, Copy)]
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
/// Legacy library projection retains its established lexical display ordering.
/// Model tools consume the typed source outcome directly, including BFS name order.
pub fn find_options(
    files: &Files,
    roots: &[Root],
    base: &str,
    options: &Options<'_>,
) -> io::Result<Value> {
    let mut report = find_checked(files, roots, base, options, &|| Ok(()))?;
    if let Some(hits) = report["matches"].as_array_mut() {
        hits.sort_by(|a, b| {
            a["path"]
                .as_str()
                .cmp(&b["path"].as_str())
                .then(a["line"].as_u64().cmp(&b["line"].as_u64()))
        });
    }
    Ok(report)
}
pub fn find_checked(
    _files: &Files,
    roots: &[Root],
    base: &str,
    options: &Options<'_>,
    authorize: &dyn Fn() -> io::Result<()>,
) -> io::Result<Value> {
    authorize()?;
    let canonical = crate::files::locate(roots, base, None, false)?.virtual_path;
    let result = engine::search(
        &mut native::Native::new(roots),
        &canonical,
        options,
        Instant::now() + Duration::from_secs(10),
        authorize,
    )?;
    Ok(
        json!({"matches": result.hits, "truncated": result.truncated, "scanned": result.files_scanned, "skipped": result.skipped, "content_file_limit": engine::MAX_CONTENT_FILE_BYTES, "stopped_because": result.stopped_because}),
    )
}
