//! Canonical approved-root enforcement. Remote access remains a separate filesystem owner.
use crate::config::{Root, js_len};
use crate::persistence::invalid;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Resolved {
    pub real: PathBuf,
    pub virtual_path: String,
    pub root: Root,
}
pub fn native_identity(path: &Path) -> String {
    let text = path
        .to_string_lossy()
        .replace("\\\\?\\UNC\\", "\\\\")
        .trim_start_matches("\\\\?\\")
        .to_string();
    #[cfg(windows)]
    {
        let lower = text.to_lowercase();
        if lower.starts_with("\\\\wsl.localhost\\") || lower.starts_with("\\\\wsl$\\") {
            let parts: Vec<_> = text.splitn(5, '\\').collect();
            if parts.len() == 5 {
                return format!(
                    "\\\\wsl.localhost\\{}\\{}",
                    parts[3].to_lowercase(),
                    parts[4]
                );
            }
        }
        text.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        text
    }
}
pub fn contained(parent: &Path, child: &Path) -> bool {
    #[cfg(not(windows))]
    {
        child.starts_with(parent)
    }
    #[cfg(windows)]
    {
        let (left, wsl_left) = native_components(parent);
        let (right, wsl_right) = native_components(child);
        if wsl_left != wsl_right || right.len() < left.len() {
            return false;
        }
        left.iter().zip(&right).enumerate().all(|(index, (a, b))| {
            let ignore_case = !wsl_left || index < 2;
            // Compare raw UTF-16 with Windows ordinal case rules. Lossy display text
            // is never an authorization key; WSL share tails remain case sensitive.
            unsafe {
                windows_sys::Win32::Globalization::CompareStringOrdinal(
                    a.as_ptr(),
                    a.len() as i32,
                    b.as_ptr(),
                    b.len() as i32,
                    i32::from(ignore_case),
                ) == 2
            }
        })
    }
}
#[cfg(windows)]
fn native_components(path: &Path) -> (Vec<Vec<u16>>, bool) {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let mut units = path
        .as_os_str()
        .encode_wide()
        .map(|u| if u == 47 { 92 } else { u })
        .collect::<Vec<_>>();
    if units.starts_with(&[92, 92, 63, 92]) {
        units.drain(..4);
        if units.starts_with(&[85, 78, 67, 92]) {
            units.splice(..4, [92, 92]);
        }
    }
    let mut wsl = false;
    if units.starts_with(&[92, 92])
        && let Some(end) = units[2..].iter().position(|u| *u == 92).map(|at| at + 2)
    {
        let host = String::from_utf16(&units[2..end]).unwrap_or_default();
        if host.eq_ignore_ascii_case("wsl$") || host.eq_ignore_ascii_case("wsl.localhost") {
            units.splice(2..end, "wsl.localhost".encode_utf16());
            wsl = true;
        }
    }
    let normalized = PathBuf::from(std::ffi::OsString::from_wide(&units));
    (
        normalized
            .components()
            .map(|c| c.as_os_str().encode_wide().collect())
            .collect(),
        wsl,
    )
}
fn real_root(root: &Root) -> io::Result<PathBuf> {
    let stored = Path::new(&root.path);
    let current = fs::canonicalize(stored)?;
    if !contained(stored, &current) || !contained(&current, stored) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Approved root changed on disk; remove and approve it again",
        ));
    }
    Ok(current)
}
fn segment(segment: &str) -> io::Result<()> {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.chars().any(|c| c < '\u{20}')
        || js_len(segment) > 255
    {
        return Err(invalid("Invalid path segment"));
    }
    #[cfg(windows)]
    {
        if segment.contains([':', '<', '>', '"', '|', '?', '*']) || segment.ends_with(['.', ' ']) {
            return Err(invalid("Invalid Windows path segment"));
        }
        let stem = segment.split('.').next().unwrap().to_ascii_lowercase();
        if ["con", "prn", "aux", "nul", "conin$", "conout$"].contains(&stem.as_str())
            || ((stem.starts_with("com") || stem.starts_with("lpt"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err(invalid("Reserved Windows device name"));
        }
    }
    Ok(())
}
fn deepest(path: &Path) -> io::Result<(PathBuf, Vec<std::ffi::OsString>)> {
    let mut current = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match fs::canonicalize(&current) {
            Ok(real) => {
                missing.reverse();
                return Ok((real, missing));
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                if fs::symlink_metadata(&current).is_ok() {
                    return Err(invalid("Path contains a link that could not be resolved"));
                }
            }
            Err(e) => return Err(e),
        }
        let leaf = current
            .file_name()
            .ok_or_else(|| invalid("Path could not be resolved safely"))?
            .to_os_string();
        missing.push(leaf);
        if !current.pop() {
            return Err(invalid("Path could not be resolved safely"));
        }
    }
}
fn validate_native(path: &str) -> io::Result<()> {
    let normalized = path.replace('\\', "/");
    for (index, part) in normalized.split('/').filter(|s| !s.is_empty()).enumerate() {
        if index == 0 && part.len() == 2 && part.ends_with(':') {
            continue;
        }
        segment(part)?;
    }
    Ok(())
}
pub fn resolve(
    roots: &[Root],
    input: &str,
    base: Option<&str>,
    allow_missing: bool,
) -> io::Result<Resolved> {
    if input.contains('\0') || js_len(input) > 4096 {
        return Err(invalid("Invalid path"));
    }
    if Path::new(input).is_absolute()
        && !(cfg!(windows) && input.starts_with('/') && !input.starts_with("//"))
    {
        let virtual_root = input
            .split('/')
            .nth(1)
            .is_some_and(|name| roots.iter().any(|r| r.name.eq_ignore_ascii_case(name)));
        if !virtual_root {
            validate_native(input)?;
            let (real, missing) = deepest(Path::new(input))?;
            let final_path = missing.iter().fold(real, |mut p, s| {
                p.push(s);
                p
            });
            for root in roots.iter().filter(|r| r.remote.is_none()) {
                let root_real = real_root(root)?;
                if contained(&root_real, &final_path) {
                    if !allow_missing && !missing.is_empty() {
                        return Err(io::Error::new(io::ErrorKind::NotFound, "Not found"));
                    }
                    let rel = relative_inside(&root_real, &final_path)?;
                    return Ok(Resolved {
                        real: final_path,
                        virtual_path: virtual_name(root, &rel),
                        root: root.clone(),
                    });
                }
            }
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Native path is not inside an approved folder",
            ));
        }
    }
    let requested = if input.starts_with('/') || (cfg!(windows) && input.starts_with('\\')) {
        input.to_string()
    } else if let Some(base) = base {
        format!("{}/{}", base.trim_end_matches(['/', '\\']), input)
    } else {
        return Err(invalid("Relative path has no active approved folder"));
    };
    let parts: Vec<_> = requested
        .split(|c| c == '/' || (cfg!(windows) && c == '\\'))
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(invalid("Path is empty"));
    }
    for part in &parts {
        segment(part)?;
    }
    let root = roots
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(parts[0]))
        .ok_or_else(|| invalid("Unknown approved root"))?;
    if root.remote.is_some() {
        return Err(invalid(
            "Server folder requires the remote filesystem owner",
        ));
    }
    let root_real = real_root(root)?;
    let candidate = parts[1..].iter().fold(root_real.clone(), |mut p, s| {
        p.push(s);
        p
    });
    let (real, missing) = deepest(&candidate)?;
    if !contained(&root_real, &real) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Path escapes its approved folder via a link",
        ));
    }
    if !allow_missing && !missing.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Not found"));
    }
    let final_path = missing.iter().fold(real, |mut p, s| {
        p.push(s);
        p
    });
    let relative = relative_inside(&root_real, &final_path)?;
    Ok(Resolved {
        real: final_path,
        virtual_path: virtual_name(root, &relative),
        root: root.clone(),
    })
}
fn relative_inside(root: &Path, target: &Path) -> io::Result<PathBuf> {
    if let Ok(relative) = target.strip_prefix(root) {
        return Ok(relative.to_path_buf());
    }
    if contained(root, target) {
        return Ok(target
            .components()
            .skip(root.components().count())
            .collect());
    }
    Err(invalid("Path is not inside approved root"))
}
fn virtual_name(root: &Root, relative: &Path) -> String {
    let suffix = relative.to_string_lossy().replace('\\', "/");
    if suffix.is_empty() {
        format!("/{}", root.name)
    } else {
        format!("/{}/{}", root.name, suffix)
    }
}
