//! Windows child-only JDK/Go discovery. Never edits the parent environment.
use cos_core::{config::js_trim, persistence::invalid};
use serde_json::{Map, Value};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, VecDeque},
    io,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};
pub type Environment = Map<String, Value>;
pub trait Probe {
    fn is_file(&self, path: &str) -> bool;
    fn directories(&self, path: &str) -> Vec<String>;
}
pub struct NativeProbe;
impl Probe for NativeProbe {
    fn is_file(&self, path: &str) -> bool {
        std::path::Path::new(path).is_file()
    }
    fn directories(&self, path: &str) -> Vec<String> {
        std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|ty| ty.is_dir()))
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect()
    }
}
#[derive(Clone)]
struct Discovery {
    java: Option<String>,
    go: Option<String>,
}
#[derive(Clone, Copy)]
struct Reachability {
    java: bool,
    go: bool,
}
pub struct Toolchains<P> {
    probe: P,
    discovered: Option<Discovery>,
    reachable: VecDeque<(String, Reachability)>,
}
fn value<'a>(env: &'a Environment, name: &str) -> Option<&'a str> {
    env.iter()
        .find(|(key, _)| key.to_lowercase() == name.to_lowercase())
        .and_then(|(_, value)| value.as_str())
}
fn set(env: &mut Environment, name: &str, text: String) {
    let keys = env
        .keys()
        .filter(|key| key.to_lowercase() == name.to_lowercase())
        .cloned()
        .collect::<Vec<_>>();
    let target = keys.first().map(String::as_str).unwrap_or(name).to_owned();
    for key in keys.iter().skip(1) {
        env.shift_remove(key);
    }
    env.insert(target, Value::String(text));
}
fn path_entries(env: &Environment) -> Vec<String> {
    value(env, "PATH")
        .unwrap_or("")
        .split(';')
        .map(|entry| {
            let entry = js_trim(entry);
            let entry = entry.strip_prefix('"').unwrap_or(entry);
            entry.strip_suffix('"').unwrap_or(entry).to_owned()
        })
        .filter(|entry| !entry.is_empty())
        .collect()
}
fn prepend(env: &mut Environment, path: &str) {
    let mut entries = path_entries(env);
    entries.retain(|entry| entry.to_lowercase() != path.to_lowercase());
    entries.insert(0, path.to_owned());
    set(env, "PATH", entries.join(";"));
}
fn join(base: &str, tail: &[&str]) -> String {
    let text = std::iter::once(base)
        .chain(tail.iter().copied())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\\")
        .replace('/', "\\");
    let mut path = PathBuf::new();
    for component in Path::new(&text).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(path.components().next_back(), Some(Component::Normal(_))) =>
            {
                path.pop();
            }
            Component::ParentDir if path.has_root() => {}
            other => path.push(other.as_os_str()),
        }
    }
    if path.as_os_str().is_empty() {
        ".".into()
    } else {
        path.to_string_lossy().into_owned()
    }
}
fn chunks(name: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut start = 0;
    let mut numeric = None;
    for (at, ch) in name.char_indices() {
        let digit = ch.is_ascii_digit();
        if numeric.is_some_and(|previous| previous != digit) {
            out.push(&name[start..at]);
            start = at;
        }
        numeric = Some(digit);
    }
    if start < name.len() {
        out.push(&name[start..]);
    }
    out
}
fn compare_versions(left: &str, right: &str) -> Ordering {
    let basename = |name: &str| {
        name.trim_end_matches(['\\', '/'])
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or("")
            .to_lowercase()
    };
    let (a, b) = (basename(left), basename(right));
    let (a, b) = (chunks(&a), chunks(&b));
    for index in 0..a.len().max(b.len()) {
        let Some(x) = a.get(index) else {
            return Ordering::Greater;
        };
        let Some(y) = b.get(index) else {
            return Ordering::Less;
        };
        let order = if x.as_bytes()[0].is_ascii_digit() && y.as_bytes()[0].is_ascii_digit() {
            let difference = x.parse::<f64>().unwrap_or(f64::INFINITY)
                - y.parse::<f64>().unwrap_or(f64::INFINITY);
            // Array.sort treats the source comparator's Infinity-Infinity (NaN) as a tie.
            let Some(order) = difference.partial_cmp(&0.0) else {
                return Ordering::Equal;
            };
            order
        } else {
            x.encode_utf16().cmp(y.encode_utf16())
        };
        if order != Ordering::Equal {
            return order.reverse();
        }
    }
    left.encode_utf16().cmp(right.encode_utf16()).reverse()
}
pub fn sort_versions(names: &mut [String]) {
    names.sort_by(|a, b| compare_versions(a, b));
}
impl<P: Probe> Toolchains<P> {
    pub fn new(probe: P) -> Self {
        Self {
            probe,
            discovered: None,
            reachable: VecDeque::new(),
        }
    }
    pub fn probe(&self) -> &P {
        &self.probe
    }
    fn path_reachability(&mut self, env: &Environment) -> Reachability {
        let key = value(env, "PATH").unwrap_or("").to_owned();
        if let Some((_, found)) = self.reachable.iter().find(|(path, _)| path == &key) {
            return *found;
        }
        let mut found = Reachability {
            java: false,
            go: false,
        };
        for entry in path_entries(env) {
            if !found.java && self.probe.is_file(&join(&entry, &["java.exe"])) {
                found.java = true;
            }
            if !found.go && self.probe.is_file(&join(&entry, &["go.exe"])) {
                found.go = true;
            }
            if found.java && found.go {
                break;
            }
        }
        self.reachable.push_back((key, found));
        if self.reachable.len() > 16 {
            self.reachable.pop_front();
        }
        found
    }
    fn first(&self, candidates: &[String], executable: &str) -> Option<String> {
        for candidate in candidates {
            if self.probe.is_file(&join(candidate, &["bin", executable])) {
                return Some(candidate.clone());
            }
            let mut children = self.probe.directories(candidate);
            sort_versions(&mut children);
            for child in children {
                if self.probe.is_file(&join(&child, &["bin", executable])) {
                    return Some(child);
                }
            }
        }
        None
    }
    fn discover(&mut self, env: &Environment) -> Discovery {
        if let Some(found) = &self.discovered {
            return found.clone();
        }
        let program_files = value(env, "ProgramFiles").unwrap_or("C:\\Program Files");
        let x86 = value(env, "ProgramFiles(x86)").unwrap_or("C:\\Program Files (x86)");
        let local = value(env, "LOCALAPPDATA").unwrap_or("");
        let mut java = vec![
            join(program_files, &["Android", "Android Studio", "jbr"]),
            join(program_files, &["Android", "Android Studio", "jre"]),
        ];
        if !local.is_empty() {
            java.push(join(local, &["Programs", "Android Studio", "jbr"]));
        }
        java.push(join(x86, &["Android", "Android Studio", "jbr"]));
        for tail in [
            vec!["Eclipse Adoptium"],
            vec!["Java"],
            vec!["Microsoft"],
            vec!["Amazon Corretto"],
            vec!["Zulu"],
            vec!["JetBrains", "Toolbox", "apps"],
        ] {
            java.push(join(program_files, &tail));
        }
        let mut go = vec![join(program_files, &["Go"])];
        if !local.is_empty() {
            go.push(join(local, &["Programs", "Go"]));
        }
        go.push("C:\\Go".into());
        let found = Discovery {
            java: self.first(&java, "javac.exe"),
            go: self.first(&go, "go.exe"),
        };
        self.discovered = Some(found.clone());
        found
    }
    pub fn ensure(&mut self, env: &mut Environment) -> Vec<String> {
        let java_missing = value(env, "JAVA_HOME").is_none();
        let go_missing = value(env, "GOROOT").is_none();
        let reachable = if java_missing || go_missing {
            self.path_reachability(env)
        } else {
            Reachability {
                java: true,
                go: true,
            }
        };
        let (java_needed, go_needed) =
            (java_missing && !reachable.java, go_missing && !reachable.go);
        if !java_needed && !go_needed {
            return vec![];
        }
        let found = self.discover(env);
        let mut added = vec![];
        if java_needed && let Some(java) = found.java {
            set(env, "JAVA_HOME", java.clone());
            prepend(env, &join(&java, &["bin"]));
            added.push(format!("JAVA_HOME={java}"));
        }
        if go_needed && let Some(go) = found.go {
            set(env, "GOROOT", go.clone());
            prepend(env, &join(&go, &["bin"]));
            added.push(format!("GOROOT={go}"));
        }
        added
    }
}

/// Merge only caller overrides and discovered changes into a child's environment.
/// Inherited secrets are never copied back after the process owner's filtering.
pub fn child_overrides(
    overrides: Option<&BTreeMap<String, String>>,
) -> io::Result<BTreeMap<String, String>> {
    static CACHE: OnceLock<Mutex<Toolchains<NativeProbe>>> = OnceLock::new();
    let mut environment = Environment::new();
    let mut unrepresentable = Vec::new();
    for (key, value) in std::env::vars_os() {
        if let Ok(key) = key.into_string() {
            match value.into_string() {
                Ok(value) => set(&mut environment, &key, value),
                Err(_)
                    if key.eq_ignore_ascii_case("JAVA_HOME")
                        || key.eq_ignore_ascii_case("GOROOT") =>
                {
                    // Presence, including an empty native value, is already the user's choice.
                    // This sentinel is never returned as an override.
                    set(&mut environment, &key, String::new());
                }
                Err(_)
                    if ["PATH", "ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
                        .iter()
                        .any(|input| key.eq_ignore_ascii_case(input)) =>
                {
                    unrepresentable.push(key.to_lowercase());
                }
                Err(_) => {}
            }
        }
    }
    if let Some(overrides) = overrides {
        for (key, value) in overrides {
            set(&mut environment, key, value.clone());
            unrepresentable.retain(|held| held != &key.to_lowercase());
        }
    }
    if !unrepresentable.is_empty() {
        // A UTF-8 projection cannot reconstruct these native UTF-16 inputs losslessly.
        // Preserve inherited values and explicit overrides rather than redirecting PATH.
        return Ok(overrides.cloned().unwrap_or_default());
    }
    let before = environment.clone();
    CACHE
        .get_or_init(|| Mutex::new(Toolchains::new(NativeProbe)))
        .lock()
        .map_err(|_| invalid("Toolchain discovery lock poisoned"))?
        .ensure(&mut environment);
    let mut changes = overrides.cloned().unwrap_or_default();
    for (key, value) in environment {
        if before.get(&key) != Some(&value)
            && let Some(value) = value.as_str()
        {
            changes.insert(key, value.to_owned());
        }
    }
    Ok(changes)
}
