use serde_json::{Value, json};
pub fn identity(remote: &Value) -> String {
    format!(
        "{}{}:{}",
        remote["host"].as_str().unwrap_or(""),
        remote["port"]
            .as_u64()
            .map(|p| format!(":{p}"))
            .unwrap_or_default(),
        remote["path"].as_str().unwrap_or("")
    )
}
pub fn parse(address: &str, folder: &str) -> Result<Value, String> {
    let mut host = address.trim().to_owned();
    if host
        .get(..6)
        .is_some_and(|s| s.eq_ignore_ascii_case("ssh://"))
    {
        host = host[6..].into();
    }
    let mut port = None;
    let re = regex::Regex::new(r"^(.*):(\d+)$").unwrap();
    if let Some(c) = re.captures(&host)
        && (!host.contains('[') || host.contains("]:"))
    {
        let p = c[2].parse::<u32>().unwrap_or(0);
        if p == 0 || p > 65535 {
            return Err("The port must be between 1 and 65535.".into());
        }
        port = Some(p);
        host = c[1].into();
    }
    if !regex::Regex::new(
        r"^(?:[A-Za-z0-9_][A-Za-z0-9._-]*@)?(?:[A-Za-z0-9_][A-Za-z0-9._-]*|\[[0-9A-Fa-f:.]+\])$",
    )
    .unwrap()
    .is_match(&host)
    {
        return Err("Enter an SSH address such as dev, me@server or me@server:2222.".into());
    }
    host = host.replace(['[', ']'], "");
    let raw = folder.trim();
    let folder = if raw.len() > 1 {
        raw.trim_end_matches('/')
    } else {
        raw
    };
    if !(folder.starts_with('/') || folder == "~" || folder.starts_with("~/"))
        || folder.chars().any(|c| c <= '\u{1f}')
        || folder.split('/').any(|p| p == "..")
    {
        return Err(
            "Enter the folder on the server as an absolute path, such as /srv/app or ~/app.".into(),
        );
    }
    let mut v = json!({"host":host,"path":folder});
    if let Some(port) = port {
        v["port"] = json!(port);
    }
    Ok(v)
}
pub fn args(remote: &Value, options: &[String], rest: &[String]) -> Vec<String> {
    let mut a = [
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=15",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ServerAliveCountMax=3",
    ]
    .map(str::to_owned)
    .to_vec();
    if let Some(port) = remote["port"].as_u64() {
        a.extend(["-p".into(), port.to_string()]);
    }
    a.extend_from_slice(options);
    a.extend(["--".into(), remote["host"].as_str().unwrap_or("").into()]);
    a.extend_from_slice(rest);
    a
}
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn folder_quote(folder: &str) -> String {
    if folder == "~" {
        "~".into()
    } else if let Some(tail) = folder.strip_prefix("~/") {
        format!("~/{}", quote(tail))
    } else {
        quote(folder)
    }
}
pub fn via_posix(script: &str) -> String {
    let mut format = String::new();
    for b in script.bytes() {
        if b.is_ascii_alphanumeric() || b" .,:/=_+@".contains(&b) {
            format.push(b as char);
        } else {
            format.push_str(&format!("\\{b:03o}"));
        }
    }
    format!("exec /bin/sh -c 'eval \"$(printf \"$0\")\"' '{format}'")
}
pub fn command(folder: &str, command: &str) -> String {
    via_posix(&format!(
        "cd -- {} && s=${{SHELL:-/bin/sh}} && case ${{s##*/}} in bash|zsh|sh) ;; *) s=$(command -v bash 2>/dev/null || command -v zsh 2>/dev/null) || s=/bin/sh ;; esac && exec \"$s\" -lc {}",
        folder_quote(folder),
        quote(command)
    ))
}
pub fn login_shell(folder: &str) -> String {
    via_posix(&format!(
        "cd -- {} && exec \"${{SHELL:-/bin/sh}}\" -l",
        folder_quote(folder)
    ))
}
pub fn program() -> String {
    if cfg!(windows) {
        let p = std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join("System32/OpenSSH/ssh.exe");
        if p.is_file() {
            return p.to_string_lossy().into_owned();
        }
    }
    "ssh".into()
}
pub fn failure(stderr: &str) -> String {
    let s = stderr.trim();
    let l = s.to_lowercase();
    if l.contains("permission denied") {
        "The server did not accept your SSH key. Add the key to your SSH agent or ~/.ssh/config so `ssh <address>` works without a password.".into()
    } else if l.contains("host key verification failed")
        || l.contains("remote host identification has changed")
    {
        "This server is not trusted yet. Run `ssh <address>` once in a terminal and confirm its host key.".into()
    } else if l.contains("could not resolve hostname") {
        "The server was not found. Check the address or its Host entry in ~/.ssh/config.".into()
    } else if l.contains("connection refused") {
        "The server refused the connection. Check that SSH runs on it and the port is right.".into()
    } else if l.contains("timed out") {
        "The server did not answer in time.".into()
    } else if l.contains("subsystem request failed") {
        "The server does not offer SFTP, which SSH projects need for files.".into()
    } else {
        s.lines()
            .filter(|l| !l.is_empty())
            .next_back()
            .unwrap_or("SSH could not connect.")
            .chars()
            .take(300)
            .collect()
    }
}
