use super::{Chunk, HEADER, Hunk, UNEXPECTED, ensure, hunk_error};
use cos_core::config::{js_len, js_trim};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    NotStarted,
    Started,
    Add,
    Delete,
    Update,
    Ended,
}
#[derive(Default)]
pub struct StreamingParser {
    buffer: String,
    mode: Mode,
    hunks: Vec<Hunk>,
    environment_id: Option<String>,
    line: usize,
    hunk_line: usize,
    units: usize,
    bytes: usize,
    breaks: usize,
}
fn trim_end(line: &str) -> &str {
    line.trim_end_matches(|c:char|matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
impl StreamingParser {
    pub fn environment_id(&self) -> Option<&str> {
        self.environment_id.as_deref()
    }
    pub fn push_delta(&mut self, delta: &str) -> Result<Vec<Hunk>, String> {
        let units = self.units.saturating_add(js_len(delta));
        let bytes = self.bytes.saturating_add(delta.len());
        let breaks = self
            .breaks
            .saturating_add(delta.bytes().filter(|b| matches!(b, b'\r' | b'\n')).count());
        crate::patch_budget::check_source_counts(bytes, breaks.saturating_add(1))
            .map_err(|e| e.to_string())?;
        if units > 4 * 1024 * 1024 {
            return Err("invalid patch: Patch exceeds 4 MiB character limit".into());
        }
        self.units = units;
        self.bytes = bytes;
        self.breaks = breaks;
        let mut start = 0;
        for (end, _) in delta.match_indices('\n') {
            self.buffer.push_str(&delta[start..end]);
            let mut line = std::mem::take(&mut self.buffer);
            if line.ends_with('\r') {
                line.pop();
            }
            self.line += 1;
            self.process(&line)?;
            start = end + 1;
        }
        self.buffer.push_str(&delta[start..]);
        Ok(self.hunks.clone())
    }
    pub fn finish(&mut self) -> Result<Vec<Hunk>, String> {
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            self.line += 1;
            if js_trim(&line) == "*** End Patch" {
                ensure(&self.hunks, js_trim(&line), self.line, self.hunk_line)?;
                self.mode = Mode::Ended;
            } else {
                self.process(&line)?;
            }
        }
        if self.mode != Mode::Ended {
            return Err("invalid patch: The last line of the patch must be '*** End Patch'".into());
        }
        Ok(self.hunks.clone())
    }
    fn process(&mut self, line: &str) -> Result<(), String> {
        let trimmed = js_trim(line);
        if self.mode == Mode::NotStarted {
            if trimmed != "*** Begin Patch" {
                return Err(
                    "invalid patch: The first line of the patch must be '*** Begin Patch'".into(),
                );
            }
            self.mode = Mode::Started;
            return Ok(());
        }
        if self.mode == Mode::Ended {
            return if trimmed.is_empty() {
                Ok(())
            } else {
                Err("invalid patch: The last line of the patch must be '*** End Patch'".into())
            };
        }
        let structural = if self.mode == Mode::Update {
            trim_end(line)
        } else {
            trimmed
        };
        if self.mode == Mode::Started
            && let Some(id) = structural.strip_prefix("*** Environment ID:")
        {
            if self.environment_id.is_some() {
                return Err(
                    "invalid patch: apply_patch environment_id cannot be specified more than once"
                        .into(),
                );
            }
            let id = js_trim(id);
            if id.is_empty() {
                return Err("invalid patch: apply_patch environment_id cannot be empty".into());
            }
            self.environment_id = Some(id.into());
            return Ok(());
        }
        let header = structural == "*** End Patch"
            || ["*** Add File: ", "*** Delete File: ", "*** Update File: "]
                .iter()
                .any(|prefix| structural.starts_with(prefix));
        if header {
            ensure(&self.hunks, structural, self.line, self.hunk_line)?;
            if structural == "*** End Patch" {
                self.mode = Mode::Ended;
            } else if let Some(path) = structural.strip_prefix("*** Add File: ") {
                self.hunks.push(Hunk::AddFile {
                    path: path.into(),
                    contents: String::new(),
                });
                self.mode = Mode::Add;
            } else if let Some(path) = structural.strip_prefix("*** Delete File: ") {
                self.hunks.push(Hunk::DeleteFile { path: path.into() });
                self.mode = Mode::Delete;
            } else if let Some(path) = structural.strip_prefix("*** Update File: ") {
                self.hunks.push(Hunk::UpdateFile {
                    path: path.into(),
                    move_path: None,
                    chunks: vec![],
                });
                self.mode = Mode::Update;
                self.hunk_line = self.line;
            }
            return Ok(());
        }
        match self.hunks.last_mut() {
            Some(Hunk::AddFile { contents, .. }) if line.starts_with('+') => {
                contents.push_str(&line[1..]);
                contents.push('\n');
                Ok(())
            }
            Some(Hunk::UpdateFile {
                move_path, chunks, ..
            }) => {
                let blank =
                    |chunk: &Chunk| chunk.old_lines.is_empty() && chunk.new_lines.is_empty();
                let context = structural == "@@" || structural.starts_with("@@ ");
                if chunks.last().is_some_and(|chunk| chunk.is_end_of_file) {
                    if structural.is_empty() {
                        return Ok(());
                    }
                    if !context {
                        return Err(hunk_error(
                            self.line,
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
                    return Ok(());
                }
                if context {
                    if chunks.last().is_some_and(blank) {
                        return Err(hunk_error(
                            self.line,
                            format!("Unexpected line found in update hunk: '{line}'. {UNEXPECTED}"),
                        ));
                    }
                    chunks.push(Chunk {
                        change_context: structural.strip_prefix("@@ ").map(str::to_owned),
                        ..Default::default()
                    });
                    return Ok(());
                }
                if structural == "*** End of File" {
                    if chunks.last().is_some_and(blank) {
                        return Err(hunk_error(
                            self.line,
                            "Update hunk does not contain any lines",
                        ));
                    }
                    if let Some(chunk) = chunks.last_mut() {
                        chunk.is_end_of_file = true;
                    }
                    return Ok(());
                }
                let prefix = line.as_bytes().first().copied().unwrap_or(b' ');
                if matches!(prefix, b' ' | b'+' | b'-') {
                    if chunks.is_empty() {
                        chunks.push(Chunk::default());
                    }
                    let chunk = chunks.last_mut().unwrap();
                    let text = line.get(1..).unwrap_or("").to_owned();
                    match prefix {
                        b'+' => chunk.new_lines.push(text),
                        b'-' => chunk.old_lines.push(text),
                        _ => {
                            chunk
                                .context_line_indices
                                .push((chunk.old_lines.len(), chunk.new_lines.len()));
                            chunk.old_lines.push(text.clone());
                            chunk.new_lines.push(text);
                        }
                    }
                    Ok(())
                } else {
                    let detail = if chunks.last().is_some_and(|chunk| !blank(chunk)) {
                        format!(
                            "Expected update hunk to start with a @@ context marker, got: '{line}'"
                        )
                    } else {
                        format!("Unexpected line found in update hunk: '{line}'. {UNEXPECTED}")
                    };
                    Err(hunk_error(self.line, detail))
                }
            }
            _ => Err(hunk_error(self.line, format!("'{trimmed}' {HEADER}"))),
        }
    }
}
