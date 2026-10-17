//! Renderer-owned terminal factory. Model calls never receive this owner or these IDs.
use crate::kernel::Kernel;
use cos_core::{config, persistence::invalid, sandbox};
use cos_exec::{
    shell, ssh,
    terminal::{self, Launch, Terminals},
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
fn permitted(kernel: &Kernel) -> bool {
    !kernel.cancel.is_cancelled()
        && config::effective_capabilities(
            &kernel.config.get(),
            if cfg!(windows) {
                "win32"
            } else if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "linux"
            },
        )["command"]
            == true
}
impl Kernel {
    pub fn terminal_owner(
        self: &Arc<Self>,
        home: &Path,
        emit: terminal::Emit,
    ) -> io::Result<Terminals> {
        let home = std::fs::canonicalize(home)?;
        if !home.is_dir() {
            return Err(invalid("Terminal home must be a directory"));
        }
        let factory = Arc::downgrade(self);
        let allowed = factory.clone();
        let validate = factory.clone();
        let factory_home = home.clone();
        Ok(Terminals::new(
            Arc::new(move |id| {
                let kernel = factory
                    .upgrade()
                    .ok_or_else(|| invalid("Terminal owner closed"))?;
                kernel.terminal_launch(id, &factory_home)
            }),
            Arc::new(move || allowed.upgrade().is_some_and(|k| permitted(&k))),
            Arc::new(move |id, launch| {
                let kernel = validate
                    .upgrade()
                    .ok_or_else(|| invalid("Terminal owner closed"))?;
                let workspace = kernel.terminal_workspace(id, &home)?;
                Ok(workspace.0 == launch.cwd && workspace.1 == launch.identity)
            }),
            emit,
        ))
    }
    fn terminal_workspace(
        &self,
        id: Option<&str>,
        home: &Path,
    ) -> io::Result<(PathBuf, Option<String>, Option<serde_json::Value>)> {
        let Some(id) = id else {
            return Ok((home.into(), None, None));
        };
        let projects = self.projects.list()?;
        let project = projects
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id && p["ungrouped"] != true)
            .ok_or_else(|| invalid("Project is no longer available"))?;
        let roots = self.roots()?;
        if let Some(remote) = project.get("remote") {
            if !roots.iter().any(|r| r.remote.as_ref() == Some(remote)) {
                return Err(invalid("Remote project root is no longer approved"));
            }
            Ok((
                home.into(),
                Some(ssh::identity(remote)),
                Some(remote.clone()),
            ))
        } else {
            let location = sandbox::resolve(
                &roots,
                project["path"]
                    .as_str()
                    .ok_or_else(|| invalid("Invalid project workspace"))?,
                None,
                false,
            )?;
            if !location.real.is_dir() {
                return Err(invalid("Project folder is unavailable"));
            }
            Ok((location.real, None, None))
        }
    }
    fn terminal_launch(&self, id: Option<&str>, home: &Path) -> io::Result<Launch> {
        let (cwd, identity, remote) = self.terminal_workspace(id, home)?;
        if let Some(remote) = remote {
            let root = self
                .roots()?
                .into_iter()
                .find(|r| r.remote.as_ref() == Some(&remote))
                .ok_or_else(|| invalid("Remote root removed"))?;
            let path = self.with_remote(&root, |r| r.resolve("", false))?;
            Ok(Launch {
                program: ssh::program(),
                args: ssh::args(&remote, &["-tt".into()], &[ssh::login_shell(&path)]),
                cwd,
                shell: "ssh".into(),
                identity,
            })
        } else {
            let shell = shell::detect(None).map_err(invalid)?;
            Ok(Launch {
                args: if shell.kind == "powershell" {
                    vec!["-NoLogo".into()]
                } else {
                    vec![]
                },
                program: shell.path,
                shell: shell.kind,
                cwd,
                identity,
            })
        }
    }
}
