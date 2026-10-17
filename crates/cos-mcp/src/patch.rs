use crate::kernel::Kernel;
use cos_core::{config, persistence::invalid, sandbox};
use cos_store::correlation::Correlation;
use cos_tools::{
    files,
    patch::{self, Hunk},
};
use serde_json::{Value, json};
use std::io;
impl Kernel {
    pub(crate) fn patch_tool(
        &self,
        roots: &[config::Root],
        base: &str,
        source: &str,
        request: &str,
        owner: Option<&Correlation>,
    ) -> io::Result<Value> {
        let mut parsed = patch::parse_typed(source).map_err(invalid)?;
        if parsed.environment_id.is_some() {
            return Err(invalid(
                "apply_patch environment selection is unavailable for this turn",
            ));
        }
        let base = if self.remote_root(roots, base).is_some() {
            base.to_owned()
        } else {
            sandbox::resolve(roots, base, None, false)?.virtual_path
        };
        let normalize = |path: &str| -> io::Result<String> {
            let candidate = if path.starts_with('/') {
                path.into()
            } else {
                format!("{}/{path}", base.trim_end_matches('/'))
            };
            if self.remote_root(roots, &candidate).is_some() {
                Ok(candidate)
            } else {
                Ok(files::locate(roots, path, Some(&base), true)?.virtual_path)
            }
        };
        let mut backend = None;
        for hunk in &mut parsed.hunks {
            let (path, target) = match hunk {
                Hunk::AddFile { path, .. } | Hunk::DeleteFile { path } => (path, None),
                Hunk::UpdateFile {
                    path, move_path, ..
                } => (path, move_path.as_mut()),
            };
            *path = normalize(path)?;
            let mut paths = vec![path.as_str()];
            if let Some(target) = target {
                *target = normalize(target)?;
                paths.push(target);
            }
            for path in paths {
                let kind = self
                    .remote_root(roots, path)
                    .map(|(r, _)| r.name.clone())
                    .unwrap_or_default();
                if backend.as_ref().is_some_and(|old| old != &kind) {
                    return Err(invalid(
                        "A patch cannot mix local and SSH roots; no edits were applied",
                    ));
                }
                backend = Some(kind);
            }
        }
        let platform = if cfg!(windows) {
            "win32"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        let capabilities = || config::effective_capabilities(&self.config.get(), platform);
        let mode = if capabilities()["edit"] == true {
            patch::FileUpdateMode::NormalizeToLf
        } else {
            patch::FileUpdateMode::PreserveLineEndings
        };
        let needed = std::cell::RefCell::new(Vec::<&str>::new());
        let authorize = |kind: &str| -> io::Result<()> {
            self.cancel.check()?;
            let current = self.admit(request)?;
            if current
                .as_ref()
                .map(|o| (&o.session_id, &o.conversation_id))
                != owner.map(|o| (&o.session_id, &o.conversation_id))
                || self.roots()? != roots
            {
                return Err(invalid(
                    "Workspace or caller authority changed during patch",
                ));
            }
            let caps = capabilities();
            if (!kind.is_empty() && caps[kind] != true)
                || needed.borrow().iter().any(|key| caps[*key] != true)
            {
                return Err(invalid("TOOL_DISABLED: apply_patch permission changed"));
            }
            if let Some(kind) = match kind {
                "create" => Some("create"),
                "edit" => Some("edit"),
                "move" => Some("move"),
                "deleteFile" => Some("deleteFile"),
                _ => None,
            } && !needed.borrow().contains(&kind)
            {
                needed.borrow_mut().push(kind);
            }
            Ok(())
        };
        for hunk in &parsed.hunks {
            match hunk {
                Hunk::AddFile { path, .. } => {
                    let exists = if let Some((root, tail)) = self.remote_root(roots, path) {
                        self.with_remote(root, |r| {
                            let real = r.resolve(&tail, true)?;
                            match r.sftp.stat(&real) {
                                Ok(_) => Ok(true),
                                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
                                Err(e) => Err(e),
                            }
                        })?
                    } else {
                        sandbox::resolve(roots, path, None, false).is_ok()
                    };
                    needed.borrow_mut().push("create");
                    if exists {
                        needed.borrow_mut().push("edit");
                    }
                }
                Hunk::DeleteFile { .. } => needed.borrow_mut().push("deleteFile"),
                Hunk::UpdateFile {
                    move_path, chunks, ..
                } => {
                    if move_path.is_none()
                        || chunks
                            .iter()
                            .any(|chunk| chunk.old_lines != chunk.new_lines)
                    {
                        needed.borrow_mut().push("edit");
                    }
                    if move_path.is_some() {
                        needed.borrow_mut().push("move");
                    }
                }
            }
        }
        authorize("")?;
        let report = if let Some(root) = roots
            .iter()
            .find(|r| Some(&r.name) == backend.as_ref() && r.remote.is_some())
        {
            for hunk in &mut parsed.hunks {
                match hunk {
                    Hunk::AddFile { path, .. } | Hunk::DeleteFile { path } => {
                        *path = self.remote_root(roots, path).unwrap().1
                    }
                    Hunk::UpdateFile {
                        path, move_path, ..
                    } => {
                        *path = self.remote_root(roots, path).unwrap().1;
                        if let Some(target) = move_path {
                            *target = self.remote_root(roots, target).unwrap().1;
                        }
                    }
                }
            }
            self.with_remote(root, |r| {
                r.apply_hunks_checked_mode(parsed.hunks, None, mode, &authorize)
            })
        } else {
            self.files
                .apply_checked_mode(roots, &base, source, None, mode, &authorize)
        };
        let report = match report {
            Ok(report) => report,
            Err(error) => {
                if let Some(detail) = error
                    .get_ref()
                    .and_then(|error| error.downcast_ref::<patch::UpdateError>())
                {
                    authorize("")?;
                    let context = if capabilities()["read"] == true {
                        detail
                            .source_context
                            .as_deref()
                            .map(|context| format!("\n\n{context}"))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    return Err(invalid(format!(
                        "apply_patch verification failed: {}{context}",
                        detail.message
                    )));
                }
                return Err(error);
            }
        };
        Ok(
            json!({"content":[{"type":"text","text":format!("Success. Updated the following files:\n{}",report["changes"].as_array().unwrap().iter().map(|v|format!("{} {}",match v["kind"].as_str().unwrap_or(""){"add"=>"A","delete"=>"D",_=>"M"},v["path"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n"))}],"structuredContent":report}),
        )
    }
}
