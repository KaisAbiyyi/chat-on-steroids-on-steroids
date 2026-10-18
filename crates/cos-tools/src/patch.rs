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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseMode {
    Strict,
    Lenient,
}
pub mod invocation;
mod streaming;
pub use streaming::StreamingParser;
mod updates;
pub use updates::{FileUpdateMode, UpdateError, seek, seek_with_mode, update, update_with_mode};

pub fn parse_typed(patch: &str) -> Result<Parsed, String> {
    parse_typed_mode(patch, ParseMode::Lenient)
}
pub fn parse_typed_mode(patch: &str, mode: ParseMode) -> Result<Parsed, String> {
    crate::patch_budget::check_source(patch).map_err(|error| error.to_string())?;
    if js_len(patch) > 4 * 1024 * 1024 {
        return Err("invalid patch: Patch exceeds 4 MiB character limit".into());
    }
    let mut lines = js_trim(patch).lines().collect::<Vec<_>>();
    let bounds = |lines: &[&str]| -> Result<(), String> {
        if lines
            .first()
            .is_some_and(|line| js_trim(line) != "*** Begin Patch")
        {
            Err("invalid patch: The first line of the patch must be '*** Begin Patch'".into())
        } else if lines
            .last()
            .is_none_or(|line| js_trim(line) != "*** End Patch")
        {
            Err("invalid patch: The last line of the patch must be '*** End Patch'".into())
        } else {
            Ok(())
        }
    };
    if let Err(original) = bounds(&lines) {
        if mode == ParseMode::Lenient
            && lines.len() >= 4
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
    let mut parser = StreamingParser::default();
    parser.push_delta(&normalized)?;
    let hunks = parser.finish()?;
    Ok(Parsed {
        hunks,
        patch: normalized,
        workdir: None,
        environment_id: parser.environment_id().map(str::to_owned),
    })
}
