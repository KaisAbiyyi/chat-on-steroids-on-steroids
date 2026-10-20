//! Read-only project Git review, matching the pinned snapshot/diff wire contract.
use crate::{
    files,
    project::{GitOutput, git_output},
};
use cos_core::{config::Root, persistence::invalid, sandbox};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::{Path, PathBuf},
};
const MAX: usize = 2000;
const SIDE: usize = 512 * 1024;
mod watch;
pub use watch::GitWatchSet;
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Change {
    status: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_path: Option<String>,
    additions: Option<u64>,
    deletions: Option<u64>,
    binary: bool,
}
fn change(status: &str, path: String, previous_path: Option<String>) -> Change {
    Change {
        status: status.into(),
        path,
        previous_path,
        additions: None,
        deletions: None,
        binary: false,
    }
}
#[derive(Serialize, Clone)]
struct Numstat {
    additions: Option<u64>,
    deletions: Option<u64>,
    binary: bool,
}
fn text(bytes: &[u8]) -> io::Result<&str> {
    std::str::from_utf8(bytes).map_err(|_| invalid("Git returned invalid UTF-8"))
}
fn safe_relative(value: &str) -> Option<String> {
    let normal = value.replace('\\', "/");
    if normal.is_empty()
        || normal.encode_utf16().count() > 4096
        || normal.starts_with('/')
        || (normal.as_bytes().get(1) == Some(&b':')
            && normal.as_bytes().get(2) == Some(&b'/')
            && normal.as_bytes()[0].is_ascii_alphabetic())
        || normal.split('/').any(|s| matches!(s, "" | "." | ".."))
    {
        None
    } else {
        Some(normal)
    }
}
fn relative(value: &str, prefix: &str) -> Option<String> {
    let safe = safe_relative(value)?;
    if prefix.is_empty() {
        Some(safe)
    } else {
        safe.strip_prefix(&format!("{prefix}/")).map(str::to_owned)
    }
}
fn fields(bytes: &[u8]) -> io::Result<Vec<&str>> {
    let mut p = text(bytes)?.split('\0').collect::<Vec<_>>();
    if p.last() == Some(&"") {
        p.pop();
    }
    Ok(p)
}
fn parse_status(bytes: &[u8], prefix: &str) -> io::Result<Vec<Change>> {
    let p = fields(bytes)?;
    let mut i = 0;
    let mut changes = vec![];
    while i < p.len() && changes.len() < MAX {
        let row = p[i];
        i += 1;
        if row.len() < 4 || !row.is_char_boundary(3) {
            continue;
        }
        let xy = &row[..2];
        let Some(path) = relative(&row[3..], prefix) else {
            continue;
        };
        if xy == "!!" {
            continue;
        }
        let kind = if xy == "??" {
            "U"
        } else if xy.contains(['R', 'C']) {
            "R"
        } else if xy.contains('D') {
            "D"
        } else if xy.contains('A') {
            "A"
        } else {
            "M"
        };
        let previous = if kind == "R" {
            let prev = p.get(i).and_then(|s| relative(s, prefix));
            i += 1;
            prev
        } else {
            None
        };
        changes.push(change(kind, path, previous));
    }
    Ok(changes)
}
fn parse_committed(bytes: &[u8], prefix: &str) -> io::Result<Vec<Change>> {
    let p = fields(bytes)?;
    let mut i = 0;
    let mut changes = vec![];
    while i < p.len() && changes.len() < MAX {
        let status = p[i];
        i += 1;
        if status.is_empty()
            || !b"ACDMRTUXB".contains(&status.as_bytes()[0])
            || !status.as_bytes()[1..].iter().all(u8::is_ascii_digit)
        {
            return Err(invalid("Git returned an invalid change status"));
        }
        let renamed = status.starts_with(['R', 'C']);
        let before = p
            .get(i)
            .ok_or_else(|| invalid("Git returned an incomplete change"))?;
        i += 1;
        let after = if renamed {
            let v = p
                .get(i)
                .ok_or_else(|| invalid("Git returned an incomplete change"))?;
            i += 1;
            v
        } else {
            before
        };
        let before = relative(before, prefix);
        let after = relative(after, prefix);
        if before.is_none() && after.is_none() {
            continue;
        }
        let kind = if renamed {
            if before.is_none() {
                "A"
            } else if after.is_none() {
                "D"
            } else {
                "R"
            }
        } else if status.starts_with('A') {
            "A"
        } else if status.starts_with('D') {
            "D"
        } else {
            "M"
        };
        let path = if kind == "D" { before.clone() } else { after }
            .ok_or_else(|| invalid("Invalid Git path"))?;
        changes.push(change(kind, path, if kind == "R" { before } else { None }));
    }
    Ok(changes)
}
fn parse_numstat(bytes: &[u8], prefix: &str) -> io::Result<BTreeMap<String, Numstat>> {
    let p = fields(bytes)?;
    let mut i = 0;
    let mut result = BTreeMap::new();
    while i < p.len() {
        let cols = p[i].splitn(3, '\t').collect::<Vec<_>>();
        i += 1;
        if cols.len() < 3 {
            continue;
        }
        let binary = cols[0] == "-" || cols[1] == "-";
        let n = Numstat {
            additions: if binary { None } else { cols[0].parse().ok() },
            deletions: if binary { None } else { cols[1].parse().ok() },
            binary,
        };
        let path = if cols[2].is_empty() {
            i += 1;
            let v = p.get(i).and_then(|s| relative(s, prefix));
            i += 1;
            v
        } else {
            relative(cols[2], prefix)
        };
        if let Some(path) = path {
            result.insert(path, n);
        }
    }
    Ok(result)
}
struct Context {
    real: PathBuf,
    prefix: String,
    virtual_path: String,
    custody: Vec<(PathBuf, same_file::Handle)>,
}
fn directory_handle(path: &Path) -> io::Result<same_file::Handle> {
    same_file::Handle::from_file(
        cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority())?.into_std_file(),
    )
}
impl Context {
    fn output(&self, args: &[&str], max: usize) -> io::Result<GitOutput> {
        git_output(&self.real, args, max)
    }
    fn run(&self, args: &[&str]) -> io::Result<Vec<u8>> {
        let out = self.output(args, 4 * 1024 * 1024)?;
        if out.code != 0 {
            return Err(invalid(if out.stderr.is_empty() {
                "Git read failed"
            } else {
                &out.stderr
            }));
        }
        Ok(out.stdout)
    }
    fn validate(&self, roots: &[Root]) -> io::Result<()> {
        let current = sandbox::resolve(roots, &self.virtual_path, None, false)?;
        if current.real != self.real {
            return Err(invalid("The project changed while Git changes were read"));
        }
        for (path, held) in &self.custody {
            if directory_handle(path)? != *held {
                return Err(invalid("The Git repository identity changed"));
            }
        }
        Ok(())
    }
    fn object_path(&self, file: &str) -> String {
        if self.prefix.is_empty() {
            file.into()
        } else {
            format!("{}/{file}", self.prefix)
        }
    }
}
fn context(roots: &[Root], path: &str) -> io::Result<Option<Context>> {
    if roots.iter().any(|r| {
        r.remote.is_some()
            && (path == format!("/{}", r.name) || path.starts_with(&format!("/{}/", r.name)))
    }) {
        return Err(invalid(
            "Git changes of a project on a server are not shown in the app yet.",
        ));
    }
    let selected = sandbox::resolve(roots, path, None, false)?;
    let inspected = git_output(
        &selected.real,
        &[
            "rev-parse",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
        ],
        4 * 1024 * 1024,
    )?;
    if inspected.code != 0 {
        if inspected
            .stderr
            .to_lowercase()
            .contains("not a git repository")
        {
            return Ok(None);
        }
        return Err(invalid(if inspected.stderr.is_empty() {
            "Git repository could not be inspected"
        } else {
            &inspected.stderr
        }));
    }
    let identity = text(&inspected.stdout)?.trim_end().replace('\r', "");
    let lines = identity.split('\n').collect::<Vec<_>>();
    if lines.len() < 3 {
        return Err(invalid("Git returned an incomplete repository identity"));
    }
    let mut paths = vec![selected.real.clone()];
    for line in &lines[..3] {
        paths.push(std::fs::canonicalize(selected.real.join(line))?);
    }
    let approved = roots
        .iter()
        .filter(|r| r.remote.is_none())
        .map(|r| std::fs::canonicalize(&r.path))
        .collect::<io::Result<Vec<_>>>()?;
    if paths
        .iter()
        .any(|p| !approved.iter().any(|a| sandbox::contained(a, p)))
    {
        return Err(invalid(
            "The Git repository metadata is outside the approved folders",
        ));
    }
    let prefix = selected
        .real
        .strip_prefix(&paths[1])
        .map_err(|_| invalid("The selected project is outside its Git worktree"))?
        .components()
        .map(|p| p.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let custody = paths
        .into_iter()
        .map(|p| directory_handle(&p).map(|h| (p, h)))
        .collect::<io::Result<Vec<_>>>()?;
    Ok(Some(Context {
        real: selected.real,
        prefix,
        virtual_path: selected.virtual_path,
        custody,
    }))
}
fn oid(bytes: &[u8]) -> io::Result<String> {
    let s = text(bytes)?.trim();
    if !(40..=64).contains(&s.len())
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("Git returned an invalid object identity"));
    }
    Ok(s.into())
}
fn branch_identity(ctx: &Context) -> io::Result<(String, Option<String>, Vec<Value>, bool)> {
    let symbolic = ctx.output(&["symbolic-ref", "--quiet", "--short", "HEAD"], 512)?;
    let head = ctx.output(&["rev-parse", "--verify", "--quiet", "HEAD"], 128)?;
    let head = if head.code == 0 {
        Some(oid(&head.stdout)?)
    } else {
        None
    };
    let current = if symbolic.code == 0 {
        text(&symbolic.stdout)?.trim().into()
    } else {
        head.as_ref().map_or("HEAD".into(), |s| s[..8].into())
    };
    let listed = ctx.run(&[
        "for-each-ref",
        "--count=1001",
        "--format=%(refname)%00%(refname:short)%00%(symref)",
        "refs/heads",
        "refs/remotes",
    ])?;
    let mut branches = vec![];
    let mut truncated = false;
    for line in text(&listed)?.lines() {
        let cols = line.trim_end_matches('\r').split('\0').collect::<Vec<_>>();
        if cols.len() < 2
            || cols.get(2).is_some_and(|s| !s.is_empty())
            || !(cols[0].starts_with("refs/heads/") || cols[0].starts_with("refs/remotes/"))
        {
            continue;
        }
        if branches.len() >= 1000 {
            truncated = true;
            break;
        }
        branches.push(json!({"ref":cols[0],"label":cols[1]}));
    }
    Ok((current, head, branches, truncated))
}
fn working_path(ctx: &Context, file: &str) -> io::Result<PathBuf> {
    let mut path = ctx.real.clone();
    for part in file.split('/') {
        path.push(part);
        let meta = std::fs::symlink_metadata(&path)?;
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let linked = meta.file_type().is_symlink();
        if linked {
            return Err(invalid("Files does not follow symbolic links or junctions"));
        }
    }
    Ok(path)
}
fn working_file(roots: &[Root], ctx: &Context, file: &str) -> io::Result<std::fs::File> {
    let path = working_path(ctx, file)?;
    let virtual_path = format!("{}/{file}", ctx.virtual_path.trim_end_matches('/'));
    let location = files::locate(roots, &virtual_path, None, false)?;
    let opened = location.open_handle()?;
    if files::handle_path(&opened)? != path || working_path(ctx, file)? != path {
        return Err(invalid(
            "The project file changed while Git changes were read",
        ));
    }
    Ok(opened)
}
fn working_bytes(roots: &[Root], ctx: &Context, path: &str) -> io::Result<(Vec<u8>, bool)> {
    let mut file = working_file(roots, ctx, path)?;
    let before = crate::project_files::file_stamp(&file)?;
    let size = file.metadata()?.len();
    let mut bytes = vec![];
    file.by_ref()
        .take(SIDE as u64 + 1)
        .read_to_end(&mut bytes)?;
    let after = crate::project_files::file_stamp(&file)?;
    let reopened = working_file(roots, ctx, path)?;
    if same_file::Handle::from_file(file.try_clone()?)? != same_file::Handle::from_file(reopened)?
        || before != after
    {
        return Err(invalid(
            "The project file changed while Git changes were read",
        ));
    }
    let large = size > SIDE as u64 || bytes.len() > SIDE;
    bytes.truncate(SIDE);
    Ok((bytes, large))
}
fn working(roots: &[Root], ctx: &Context, file: &str) -> io::Result<(Option<String>, bool)> {
    let (bytes, large) = working_bytes(roots, ctx, file)?;
    Ok((
        if large || bytes.contains(&0) {
            None
        } else {
            String::from_utf8(bytes).ok()
        },
        large,
    ))
}
fn snapshot_inner(
    roots: &[Root],
    path: &str,
    project: &str,
    base: Option<&str>,
) -> io::Result<Value> {
    let Some(ctx) = context(roots, path)? else {
        return Ok(
            json!({"projectId":project,"state":"not-repository","changes":[],"truncated":false,"revision":""}),
        );
    };
    let (current, head, branches, branches_truncated) = branch_identity(&ctx)?;
    let (mut changes, stats, comparison) = if let Some(base) = base.filter(|s| !s.is_empty()) {
        let branch = branches
            .iter()
            .find(|v| v["ref"] == base)
            .ok_or_else(|| invalid("The selected comparison branch is no longer available"))?;
        let head = head
            .as_ref()
            .ok_or_else(|| invalid("This branch has no commits to compare"))?;
        let base_oid = oid(&ctx.run(&["rev-parse", "--verify", "--quiet", base])?)?;
        let ancestor = ctx.output(&["merge-base", &base_oid, head], 128)?;
        if ancestor.code != 0 {
            return Err(invalid("These branches have no common ancestor"));
        }
        let ancestor = oid(&ancestor.stdout)?;
        let status = ctx.run(&[
            "diff",
            "--name-status",
            "-z",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames=50%",
            &ancestor,
            head,
            "--",
            ".",
        ])?;
        let stats = ctx.run(&[
            "diff",
            "--numstat",
            "-z",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames=50%",
            &ancestor,
            head,
            "--",
            ".",
        ])?;
        (
            parse_committed(&status, &ctx.prefix)?,
            parse_numstat(&stats, &ctx.prefix)?,
            Some(
                json!({"ref":branch["ref"],"label":branch["label"],"baseOid":ancestor,"headOid":head}),
            ),
        )
    } else {
        let status = ctx.run(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=dirty",
            "--find-renames=50%",
            "--",
            ".",
        ])?;
        let stats = ctx.run(&[
            "diff",
            "--numstat",
            "-z",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames=50%",
            if head.is_some() { "HEAD" } else { "--cached" },
            "--",
            ".",
        ])?;
        (
            parse_status(&status, &ctx.prefix)?,
            parse_numstat(&stats, &ctx.prefix)?,
            None,
        )
    };
    for change in &mut changes {
        if let Some(n) = stats.get(&change.path) {
            change.additions = n.additions;
            change.deletions = n.deletions;
            change.binary = n.binary;
        }
        if comparison.is_none()
            && (change.status == "U" || (change.status == "A" && !stats.contains_key(&change.path)))
        {
            match working_bytes(roots, &ctx, &change.path) {
                Ok((bytes, large)) => {
                    if bytes.contains(&0) {
                        change.binary = true;
                    } else if !large {
                        match String::from_utf8(bytes) {
                            Ok(text) => {
                                change.additions = Some(if text.is_empty() {
                                    0
                                } else {
                                    text.split('\n').count() as u64
                                        - u64::from(text.ends_with('\n'))
                                });
                                change.deletions = Some(0);
                            }
                            Err(_) => change.binary = true,
                        }
                    }
                }
                Err(_) => change.binary = true,
            }
        }
    }
    // The upstream digest covered only row statistics. Include the base and
    // native file identity/change times so equal-line-count edits and HEAD
    // changes cannot reuse a stale Review token. The token remains opaque.
    let mut stamps = Vec::new();
    if comparison.is_none() {
        for change in &changes {
            if change.status == "D" {
                stamps.push(Value::Null);
                continue;
            }
            let meta = std::fs::symlink_metadata(ctx.real.join(&change.path))?;
            if meta.is_file() && !meta.file_type().is_symlink() {
                match working_file(roots, &ctx, &change.path) {
                    Ok(file) => stamps.push(crate::project_files::file_stamp(&file)?),
                    Err(_) => stamps.push(json!([
                        "unreadable",
                        meta.len(),
                        meta.modified()?
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|t| t.as_nanos().to_string())
                    ])),
                }
            } else {
                stamps.push(json!([
                    "non-regular",
                    meta.len(),
                    meta.modified()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|t| t.as_nanos().to_string())
                ]));
            }
        }
    }
    ctx.validate(roots)?;
    let changes = serde_json::to_value(changes)?;
    let revision = files::digest(&serde_json::to_vec(&if let Some(c) = &comparison {
        json!([c, changes])
    } else {
        json!([current, head, changes, stamps])
    })?);
    let mut result = json!({"projectId":project,"state":"ready","truncated":changes.as_array().unwrap().len()>=MAX,"changes":changes,"revision":revision,"currentBranch":current,"branches":branches,"branchesTruncated":branches_truncated});
    if let Some(comparison) = comparison {
        result["comparison"] = comparison;
    }
    Ok(result)
}
pub fn snapshot(
    roots: &[Root],
    path: &str,
    project: &str,
    base: Option<&str>,
) -> io::Result<Value> {
    Ok(snapshot_inner(roots,path,project,base).unwrap_or_else(|e|json!({"projectId":project,"state":"unavailable","changes":[],"truncated":false,"revision":"","message":e.to_string()})))
}
fn head_file(ctx: &Context, file: &str, oid: &str) -> io::Result<(Option<String>, bool)> {
    let object = format!("{oid}:{}", ctx.object_path(file));
    let size = ctx.output(&["cat-file", "-s", &object], 128)?;
    if size.code != 0 {
        return Ok((Some(String::new()), false));
    }
    let size = text(&size.stdout)?
        .trim()
        .parse::<usize>()
        .unwrap_or(usize::MAX);
    if size > SIDE {
        return Ok((None, true));
    }
    let bytes = ctx.run(&["cat-file", "blob", &object])?;
    if bytes.len() > SIDE {
        return Ok((None, true));
    }
    Ok((
        if bytes.contains(&0) {
            None
        } else {
            String::from_utf8(bytes).ok()
        },
        false,
    ))
}
pub fn diff(
    roots: &[Root],
    path: &str,
    project: &str,
    file: &str,
    base: Option<&str>,
    expected_revision: Option<&str>,
) -> io::Result<Value> {
    let s = snapshot(roots, path, project, base)?;
    if s["state"] != "ready" {
        return Err(invalid(
            s["message"]
                .as_str()
                .unwrap_or("This folder is not a Git repository"),
        ));
    }
    if expected_revision
        .filter(|s| !s.is_empty())
        .is_some_and(|r| s["revision"] != r)
    {
        return Err(invalid("The Git comparison changed; refresh Review"));
    }
    let c = s["changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["path"] == file)
        .ok_or_else(|| invalid("This file is no longer changed"))?;
    let file = safe_relative(file).ok_or_else(|| invalid("Invalid project file path"))?;
    let ctx =
        context(roots, path)?.ok_or_else(|| invalid("This folder is not a Git repository"))?;
    let (base_text, base_large) = if c["status"] == "A" || c["status"] == "U" {
        (Some(String::new()), false)
    } else {
        head_file(
            &ctx,
            c["previousPath"].as_str().unwrap_or(&file),
            s["comparison"]["baseOid"].as_str().unwrap_or("HEAD"),
        )?
    };
    let (current_text, current_large) = if c["status"] == "D" {
        (Some(String::new()), false)
    } else if let Some(head) = s["comparison"]["headOid"].as_str() {
        head_file(&ctx, &file, head)?
    } else {
        working(roots, &ctx, &file)?
    };
    let too_large = base_large || current_large;
    let binary =
        c["binary"] == true || (!too_large && (base_text.is_none() || current_text.is_none()));
    ctx.validate(roots)?;
    let latest = snapshot(roots, path, project, base)?;
    if latest["state"] != "ready" || latest["revision"] != s["revision"] {
        return Err(invalid("The Git comparison changed; refresh Review"));
    }
    let mut result = json!({"projectId":project,"status":c["status"],"path":file,"additions":c["additions"],"deletions":c["deletions"],"binary":binary,"tooLarge":too_large,"baseText":if binary||too_large{None}else{base_text},"currentText":if binary||too_large{None}else{current_text}});
    if let Some(previous) = c.get("previousPath") {
        result["previousPath"] = previous.clone();
    }
    if too_large || binary {
        result["note"] = json!(if too_large {
            "This diff is too large to display."
        } else {
            "Binary diff · preview unavailable"
        });
    }
    Ok(result)
}
pub(crate) fn legacy_snapshot(roots: &[Root], path: &str) -> io::Result<Value> {
    let ctx =
        context(roots, path)?.ok_or_else(|| invalid("This folder is not a Git repository"))?;
    let bytes = crate::project::git_command(
        &ctx.real,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut records = bytes.split(|b| *b == 0).filter(|p| !p.is_empty());
    let mut files = vec![];
    while let Some(row) = records.next() {
        if row.len() < 3 {
            continue;
        }
        let status = String::from_utf8_lossy(&row[..2]);
        let path = String::from_utf8_lossy(&row[3..]);
        let mut item = json!({"path":path,"status":status,"staged":row[0]!=b' '&&row[0]!=b'?',"untracked":status=="??"});
        if matches!(row[0], b'R' | b'C') || matches!(row[1], b'R' | b'C') {
            item["originalPath"] = json!(records.next().map(String::from_utf8_lossy));
        }
        files.push(item);
        if files.len() >= 2000 {
            break;
        }
    }
    let branch =
        crate::project::git_command(&ctx.real, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|s| String::from_utf8_lossy(&s).trim().to_string());
    let revision = crate::project::git_command(&ctx.real, &["rev-parse", "--verify", "HEAD"])
        .ok()
        .map(|s| String::from_utf8_lossy(&s).trim().to_string());
    ctx.validate(roots)?;
    Ok(
        json!({"status":"ready","branch":branch,"revision":revision,"truncated":files.len()>=2000,"files":files}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_git_projection_oracles() {
        let cases: Value = serde_json::from_str(include_str!(
            "../../../fixtures/upstream/phase1-6-closure/git.json"
        ))
        .unwrap();
        for c in cases.as_array().unwrap() {
            let raw = c["raw"].as_str().unwrap_or("").as_bytes();
            let prefix = c["prefix"].as_str().unwrap_or("");
            let actual = match c["kind"].as_str().unwrap() {
                "status" => serde_json::to_value(parse_status(raw, prefix).unwrap()).unwrap(),
                "committed" => serde_json::to_value(parse_committed(raw, prefix).unwrap()).unwrap(),
                "numstat" => serde_json::to_value(parse_numstat(raw, prefix).unwrap()).unwrap(),
                "relative" => json!(safe_relative(c["value"].as_str().unwrap())),
                _ => unreachable!(),
            };
            assert_eq!(actual, c["expected"], "{c}");
        }
    }
}
