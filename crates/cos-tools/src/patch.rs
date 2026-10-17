use cos_core::config::{js_len, js_trim};
use serde::{Deserialize, Serialize};
use serde_json::Value;
const UNEXPECTED: &str =
    "Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)";
const HEADER: &str = "is not a valid hunk header. Valid hunk headers: '*** Add File: {path}', '*** Delete File: {path}', '*** Update File: {path}'";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub change_context: Option<String>,
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
    pub context_line_indices: Vec<(usize, usize)>,
    pub is_end_of_file: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Hunk {
    AddFile {
        path: String,
        contents: String,
    },
    DeleteFile {
        path: String,
    },
    UpdateFile {
        path: String,
        #[serde(rename = "movePath")]
        move_path: Option<String>,
        chunks: Vec<Chunk>,
    },
}
impl Hunk {
    pub fn path(&self) -> &str {
        match self {
            Self::AddFile { path, .. }
            | Self::DeleteFile { path }
            | Self::UpdateFile { path, .. } => path,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub hunks: Vec<Hunk>,
    pub patch: String,
    pub workdir: Option<String>,
    pub environment_id: Option<String>,
}
fn hunk_error(line: usize, detail: impl AsRef<str>) -> String {
    format!("invalid hunk at line {line}, {}", detail.as_ref())
}
fn ensure(hunks: &[Hunk], line: &str, n: usize, hunk_line: usize) -> Result<(), String> {
    if let Some(Hunk::UpdateFile { path, chunks, .. }) = hunks.last() {
        if chunks.is_empty() {
            return Err(hunk_error(
                hunk_line,
                format!("Update file hunk for path '{path}' is empty"),
            ));
        }
        if chunks
            .last()
            .is_some_and(|c| c.old_lines.is_empty() && c.new_lines.is_empty())
        {
            return Err(hunk_error(
                n,
                if line == "*** End Patch" {
                    "Update hunk does not contain any lines".into()
                } else {
                    format!("Unexpected line found in update hunk: '{line}'. {UNEXPECTED}")
                },
            ));
        }
    }
    Ok(())
}
pub fn parse(patch: &str) -> Result<Value, String> {
    serde_json::to_value(parse_typed(patch)?).map_err(|e| e.to_string())
}
pub fn parse_typed(patch: &str) -> Result<Parsed, String> {
    crate::patch_budget::check_source(patch).map_err(|e| e.to_string())?;
    if js_len(patch) > 4 * 1024 * 1024 {
        return Err("invalid patch: Patch exceeds 4 MiB character limit".into());
    }
    let mut lines = js_trim(patch).lines().collect::<Vec<_>>();
    let bounds = |lines: &[&str]| -> Result<(), String> {
        if lines
            .first()
            .is_some_and(|l| js_trim(l) != "*** Begin Patch")
        {
            Err("invalid patch: The first line of the patch must be '*** Begin Patch'".into())
        } else if lines.last().is_none_or(|l| js_trim(l) != "*** End Patch") {
            Err("invalid patch: The last line of the patch must be '*** End Patch'".into())
        } else {
            Ok(())
        }
    };
    if let Err(original) = bounds(&lines) {
        if lines.len() >= 4
            && matches!(lines[0], "<<EOF" | "<<'EOF'" | "<<\"EOF\"")
            && lines.last().unwrap().ends_with("EOF")
        {
            lines = lines[1..lines.len() - 1].to_vec();
            bounds(&lines)?;
        } else {
            return Err(original);
        }
    }
    let normalized = lines.join("\n");
    let mut hunks = vec![];
    let mut environment_id = None;
    let mut hunk_line = 0;
    let mut ended = false;
    for (i, line) in lines.iter().enumerate().skip(1) {
        let n = i + 1;
        let trimmed = js_trim(line);
        if ended {
            if !trimmed.is_empty() {
                return Err(
                    "invalid patch: The last line of the patch must be '*** End Patch'".into(),
                );
            }
            continue;
        }
        if hunks.is_empty()
            && let Some(id) = trimmed.strip_prefix("*** Environment ID:")
        {
            if environment_id.is_some() {
                return Err(
                    "invalid patch: apply_patch environment_id cannot be specified more than once"
                        .into(),
                );
            }
            let id = js_trim(id);
            if id.is_empty() {
                return Err("invalid patch: apply_patch environment_id cannot be empty".into());
            }
            environment_id = Some(id.into());
            continue;
        }
        let structural = if matches!(hunks.last(), Some(Hunk::UpdateFile { .. })) {
            line.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        } else {
            trimmed
        };
        let header = structural == "*** End Patch"
            || ["*** Add File: ", "*** Delete File: ", "*** Update File: "]
                .iter()
                .any(|p| structural.starts_with(p));
        if header {
            ensure(&hunks, structural, n, hunk_line)?;
            if structural == "*** End Patch" {
                ended = true;
                continue;
            }
            if let Some(path) = structural.strip_prefix("*** Add File: ") {
                hunks.push(Hunk::AddFile {
                    path: path.into(),
                    contents: String::new(),
                });
            }
            if let Some(path) = structural.strip_prefix("*** Delete File: ") {
                hunks.push(Hunk::DeleteFile { path: path.into() });
            }
            if let Some(path) = structural.strip_prefix("*** Update File: ") {
                hunk_line = n;
                hunks.push(Hunk::UpdateFile {
                    path: path.into(),
                    move_path: None,
                    chunks: vec![],
                });
            }
            continue;
        }
        match hunks.last_mut() {
            Some(Hunk::AddFile { contents, .. }) if line.starts_with('+') => {
                contents.push_str(&line[1..]);
                contents.push('\n');
            }
            Some(Hunk::UpdateFile {
                move_path, chunks, ..
            }) => {
                let blank = |c: &Chunk| c.old_lines.is_empty() && c.new_lines.is_empty();
                let context = structural == "@@" || structural.starts_with("@@ ");
                if chunks.last().is_some_and(|c| c.is_end_of_file) {
                    if structural.is_empty() {
                        continue;
                    }
                    if !context {
                        return Err(hunk_error(
                            n,
                            format!(
                                "Expected update hunk to start with a @@ context marker, got: '{line}'"
                            ),
                        ));
                    }
                }
                if chunks.is_empty()
                    && move_path.is_none()
                    && let Some(path) = structural.strip_prefix("*** Move to: ")
                {
                    *move_path = Some(path.into());
                    continue;
                }
                if context {
                    if chunks.last().is_some_and(blank) {
                        return Err(hunk_error(
                            n,
                            format!("Unexpected line found in update hunk: '{line}'. {UNEXPECTED}"),
                        ));
                    }
                    chunks.push(Chunk {
                        change_context: structural.strip_prefix("@@ ").map(str::to_owned),
                        ..Default::default()
                    });
                    continue;
                }
                if structural == "*** End of File" {
                    if chunks.last().is_some_and(blank) {
                        return Err(hunk_error(n, "Update hunk does not contain any lines"));
                    }
                    if let Some(c) = chunks.last_mut() {
                        c.is_end_of_file = true;
                    }
                    continue;
                }
                let prefix = line.as_bytes().first().copied().unwrap_or(b' ');
                if matches!(prefix, b' ' | b'+' | b'-') {
                    if chunks.is_empty() {
                        chunks.push(Chunk::default());
                    }
                    let c = chunks.last_mut().unwrap();
                    let text = line.get(1..).unwrap_or("").to_string();
                    match prefix {
                        b'+' => c.new_lines.push(text),
                        b'-' => c.old_lines.push(text),
                        _ => {
                            c.context_line_indices
                                .push((c.old_lines.len(), c.new_lines.len()));
                            c.old_lines.push(text.clone());
                            c.new_lines.push(text);
                        }
                    }
                } else {
                    let detail = if chunks.last().is_some_and(|c| !blank(c)) {
                        format!(
                            "Expected update hunk to start with a @@ context marker, got: '{line}'"
                        )
                    } else {
                        format!("Unexpected line found in update hunk: '{line}'. {UNEXPECTED}")
                    };
                    return Err(hunk_error(n, detail));
                }
            }
            _ => return Err(hunk_error(n, format!("'{trimmed}' {HEADER}"))),
        }
    }
    Ok(Parsed {
        hunks,
        patch: normalized,
        workdir: None,
        environment_id,
    })
}
mod updates;
pub use updates::{FileUpdateMode, UpdateError, seek, seek_with_mode, update, update_with_mode};
