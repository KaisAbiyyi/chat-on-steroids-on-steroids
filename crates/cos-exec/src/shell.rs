use serde_json::{Value, json};
use std::{
    env,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Shell {
    pub kind: String,
    pub path: String,
}
pub fn kind(path: &str) -> Option<&'static str> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    match stem {
        "zsh" => Some("zsh"),
        "bash" => Some("bash"),
        "sh" => Some("sh"),
        "cmd" => Some("cmd"),
        "pwsh" | "powershell" => Some("powershell"),
        _ => None,
    }
}
pub fn which(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() && p.is_file() {
        return Some(p.into());
    }
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        for ext in if cfg!(windows) {
            vec!["", ".exe", ".cmd", ".bat"]
        } else {
            vec![""]
        } {
            let p = dir.join(format!("{name}{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}
pub fn detect(provided: Option<&str>) -> Result<Shell, String> {
    if let Some(path) = provided {
        let kind = kind(path).ok_or("Unsupported shell")?;
        let path = which(path).ok_or("Shell executable was not found")?;
        return Ok(Shell {
            kind: kind.into(),
            path: path.to_string_lossy().into_owned(),
        });
    }
    let mut names = vec![];
    if cfg!(windows) {
        names.extend(["pwsh".to_string(), "powershell".into()]);
    } else {
        if let Ok(s) = env::var("SHELL")
            && kind(&s).is_some()
        {
            names.push(s);
        }
        names.extend(if cfg!(target_os = "macos") {
            ["zsh".into(), "bash".into()]
        } else {
            ["bash".into(), "zsh".into()]
        });
        names.push("sh".into());
    }
    for name in names {
        if let Some(path) = which(&name) {
            return Ok(Shell {
                kind: kind(&name).unwrap().into(),
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    if cfg!(windows) {
        let p = PathBuf::from(env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        if p.is_file() {
            return Ok(Shell {
                kind: "powershell".into(),
                path: p.to_string_lossy().into_owned(),
            });
        }
    }
    Err("No supported shell was found".into())
}
impl Shell {
    pub fn args(&self, command: &str, login: bool) -> Vec<String> {
        match self.kind.as_str() {
            "powershell" => {
                let mut a = vec![];
                if !login {
                    a.push("-NoProfile".into());
                }
                a.extend(["-Command".into(),format!("try {{ [Console]::OutputEncoding=[System.Text.Encoding]::UTF8 }} catch {{}}\n{command}")]);
                a
            }
            "cmd" => vec!["/c".into(), command.into()],
            _ => vec![
                if login { "-lc".into() } else { "-c".into() },
                command.into(),
            ],
        }
    }
}
pub fn environment() -> Value {
    json!({"NO_COLOR":"1","TERM":"dumb","LANG":if cfg!(target_os="macos"){"en_US.UTF-8"}else{"C.UTF-8"},"LC_CTYPE":if cfg!(target_os="macos"){"en_US.UTF-8"}else{"C.UTF-8"},"LC_ALL":if cfg!(target_os="macos"){"en_US.UTF-8"}else{"C.UTF-8"},"COLORTERM":"","PAGER":"cat","GIT_PAGER":"cat","GH_PAGER":"cat","CODEX_CI":"1"})
}
