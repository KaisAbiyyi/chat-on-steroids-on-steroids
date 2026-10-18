use crate::kernel::{Kernel, ok};
use cos_core::{
    config::{self, Root},
    persistence::invalid,
};
use cos_store::correlation::Correlation;
use cos_tools::search::{
    Options,
    engine::{self, Backend, StopReason},
    native::Native,
    server::Server,
};
use serde_json::Value;
use std::{
    io,
    time::{Duration, Instant},
};

impl Kernel {
    pub(crate) fn find_tool(
        &self,
        roots: &[Root],
        args: &Value,
        request: &str,
        owner: Option<&Correlation>,
    ) -> io::Result<Value> {
        let authorize = || {
            self.cancel.check()?;
            let current = self.admit(request)?;
            if current
                .as_ref()
                .map(|o| (&o.session_id, &o.conversation_id))
                != owner.map(|o| (&o.session_id, &o.conversation_id))
                || self.roots()? != roots
            {
                return Err(invalid(
                    "Workspace or caller authority changed during search",
                ));
            }
            let platform = if cfg!(windows) {
                "win32"
            } else if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "linux"
            };
            if config::effective_capabilities(&self.config.get(), platform)["search"] != true {
                return Err(invalid(
                    "TOOL_DISABLED: find is disabled by the current Chat On Steroids permissions.",
                ));
            }
            Ok(())
        };
        authorize()?;
        let excludes = args["exclude"].as_array().map(|values| {
            values
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        });
        let options = Options {
            query: args["query"].as_str().unwrap(),
            content: args["mode"] == "content",
            glob: args["include"].as_str(),
            exclude: excludes.as_deref(),
            limit: args["max_results"].as_u64().unwrap_or(50) as usize,
            case_sensitive: args["case_sensitive"] == true,
            regex: args["regex"] == true,
        };
        if options.regex && !options.content {
            return Err(invalid("regex=true is only valid with mode=content"));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let explicit = args["path"].as_str().filter(|path| !path.is_empty());
        let mut scopes = vec![];
        let mut unreachable = vec![];
        if let Some(path) = explicit {
            scopes.push(if self.remote_root(roots, path).is_some() {
                path.into()
            } else {
                cos_tools::files::locate(roots, path, None, false)?.virtual_path
            });
        } else {
            // Resolve every approved root before scanning, in its configured order.
            for root in roots {
                authorize()?;
                let path = format!("/{}", root.name);
                if root.remote.is_some() {
                    match self.with_remote(root, |remote| remote.resolve("", false).map(|_| ())) {
                        Ok(()) => scopes.push(path),
                        Err(error) => {
                            authorize()?;
                            unreachable.push(format!(
                                "{path} ({})",
                                cos_core::logging::redact_credentials(&error.to_string())
                            ));
                        }
                    }
                } else {
                    scopes.push(cos_tools::files::locate(roots, &path, None, false)?.virtual_path);
                }
            }
        }
        if scopes.is_empty() {
            return Err(invalid("No folders are approved"));
        }
        let mut hits = vec![];
        let mut reasons = vec![];
        let mut scanned = 0;
        let mut elapsed = 0;
        for scope in scopes {
            authorize()?;
            if Instant::now() >= deadline {
                if !reasons.contains(&"time") {
                    reasons.push("time");
                }
                break;
            }
            if hits.len() >= options.limit {
                break;
            }
            let remote_root = self.remote_root(roots, &scope).map(|(root, _)| root);
            let search = |backend: &mut dyn Backend| {
                let file =
                    explicit.is_some() && backend.metadata(&scope)?.kind == engine::Kind::File;
                engine::search(
                    backend,
                    &scope,
                    &Options {
                        limit: options.limit - hits.len(),
                        ..options
                    },
                    deadline,
                    &authorize,
                )
                .map(|outcome| (file, outcome))
            };
            let searched = if let Some(root) = remote_root {
                self.with_remote(root, |remote| {
                    search(&mut Server::new(remote, format!("/{}", root.name)))
                })
            } else {
                search(&mut Native::new(roots))
            };
            let (file, outcome) = match searched {
                Ok(outcome) => outcome,
                Err(error) if remote_root.is_some() && explicit.is_none() => {
                    authorize()?;
                    unreachable.push(format!(
                        "{scope} ({})",
                        cos_core::logging::redact_credentials(&error.to_string())
                    ));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if file {
                if outcome.stopped_because == Some(StopReason::Size) {
                    return Err(invalid(
                        "File was not searched: content search skips files over 2.0 MB. Narrow it with read start_line/end_line, or use exec_command when command execution is available.",
                    ));
                }
                authorize()?;
                return Ok(ok(format_hits(outcome.hits)));
            }
            scanned += outcome.files_scanned;
            elapsed += outcome.elapsed_ms;
            if let Some(reason) = outcome.stopped_because
                && !reasons.contains(&reason.as_str())
            {
                reasons.push(reason.as_str());
            }
            hits.extend(outcome.hits);
        }
        let count = hits.len();
        let mut text = format!(
            "{}\n\nfiles_scanned: {scanned}\nelapsed_ms: {elapsed}\nresults_returned: {count}",
            format_hits(hits)
        );
        if options.content {
            text.push_str("\ncontent_file_limit: 2.0 MB");
        }
        if !reasons.is_empty() {
            text.push_str(&format!("\ntruncated: {}", reasons.join(",")));
        }
        if !unreachable.is_empty() {
            text.push_str(&format!("\nnot_searched: {}", unreachable.join("; ")));
        }
        authorize()?;
        Ok(ok(text))
    }
}
fn format_hits(hits: Vec<engine::Hit>) -> String {
    if hits.is_empty() {
        return "No matches".into();
    }
    format!(
        "{} matches\n{}",
        hits.len(),
        hits.into_iter()
            .map(|hit| {
                if let Some(line) = hit.line {
                    format!("{}:{line}: {}", hit.path, hit.text.unwrap_or_default())
                } else {
                    hit.path
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    )
}
