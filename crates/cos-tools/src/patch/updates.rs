use super::Chunk;
use crate::text;
use cos_core::config::{js_len, js_trim};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileUpdateMode {
    #[default]
    NormalizeToLf,
    PreserveLineEndings,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateError {
    pub message: String,
    pub source_context: Option<String>,
}
impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for UpdateError {}
fn trim_end(value: &str) -> &str {
    value.trim_end_matches(|c:char|matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
fn normalize(value: &str) -> String {
    js_trim(value)
        .chars()
        .map(|c| match c {
            '‐' | '‑' | '‒' | '–' | '—' | '―' | '−' => '-',
            '‘' | '’' | '‚' | '‛' => '\'',
            '“' | '”' | '„' | '‟' => '"',
            '\u{a0}' | '\u{2002}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => ' ',
            _ => c,
        })
        .collect()
}
pub fn seek(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    seek_with_mode(
        lines,
        pattern,
        start,
        eof,
        FileUpdateMode::PreserveLineEndings,
    )
}
pub fn seek_with_mode(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
    mode: FileUpdateMode,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }
    let from = if eof {
        let end = lines.len() - pattern.len();
        if mode == FileUpdateMode::NormalizeToLf {
            end
        } else {
            start.max(end)
        }
    } else {
        start
    };
    for pass in 0..4 {
        for at in from..=lines.len() - pattern.len() {
            if lines[at..at + pattern.len()]
                .iter()
                .zip(pattern)
                .all(|(a, b)| match pass {
                    0 => a == b,
                    1 => trim_end(a) == trim_end(b),
                    2 => js_trim(a) == js_trim(b),
                    _ => normalize(a) == normalize(b),
                })
            {
                return Some(at);
            }
        }
    }
    None
}
fn clip(value: &str) -> String {
    if js_len(value) <= 240 {
        return value.into();
    }
    // The workspace-wide lone-surrogate boundary remains an explicit parity gate.
    format!(
        "{}…",
        String::from_utf16_lossy(&value.encode_utf16().take(240).collect::<Vec<_>>())
    )
}
fn failure(
    heading: String,
    lines: &[String],
    pattern: &[String],
    start: usize,
    mode: FileUpdateMode,
    context: Option<usize>,
) -> UpdateError {
    let earlier = if start > 0 {
        seek_with_mode(lines, pattern, 0, false, mode)
    } else {
        None
    };
    let out_of_order = earlier.is_some_and(|index| index < start);
    let guidance = if out_of_order {
        format!(
            "Matching text exists at line {}, before the current search position at line {}. Put edits in file order and avoid overlapping hunks.",
            earlier.unwrap() + 1,
            start + 1
        )
    } else {
        "Use the current file text as patch context; do not reconstruct or reformat the old lines."
            .into()
    };
    let mut expected = pattern
        .iter()
        .take(8)
        .map(|line| clip(line))
        .collect::<Vec<_>>()
        .join("\n");
    if pattern.len() > 8 {
        expected.push_str("\n… (expected text truncated)");
    }
    let mut anchor = if out_of_order { earlier } else { context };
    if anchor.is_none() {
        for candidate in pattern.iter().take(12) {
            let candidate = js_trim(candidate);
            if js_len(candidate) < 8 {
                continue;
            }
            let mut matches = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| js_trim(line) == candidate);
            if let Some((first, _)) = matches.next()
                && matches.next().is_none()
            {
                anchor = Some(first);
                break;
            }
        }
    }
    let source_context=anchor.map(|anchor|{
        let from=anchor.saturating_sub(2);
        let excerpt=lines.iter().skip(from).take(8).enumerate().map(|(index,line)|format!("{}\t{}",from+index+1,clip(line))).collect::<Vec<_>>().join("\n");
        format!("Source excerpt from the patch verification snapshot (line numbers are not file content):\n{excerpt}")
    });
    UpdateError {
        message: format!("{heading}\n{guidance}\nExpected text:\n{expected}"),
        source_context,
    }
}
/// Compatibility helper for callers explicitly preserving source line endings.
pub fn update(contents: &str, path: &str, chunks: &[Chunk]) -> Result<String, String> {
    update_with_mode(contents, path, chunks, FileUpdateMode::PreserveLineEndings)
        .map_err(|e| e.message)
}
pub fn update_with_mode(
    contents: &str,
    path: &str,
    chunks: &[Chunk],
    mode: FileUpdateMode,
) -> Result<String, UpdateError> {
    let (mark, contents) = contents
        .strip_prefix('\u{feff}')
        .map_or(("", contents), |text| ("\u{feff}", text));
    let lines = if mode == FileUpdateMode::NormalizeToLf {
        let mut lines = contents.split('\n').map(str::to_owned).collect::<Vec<_>>();
        if lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines
    } else {
        text::lines(contents)
    };
    let mut replacements = vec![];
    let mut position = 0;
    for chunk in chunks {
        let mut context_index = None;
        if let Some(context) = &chunk.change_context {
            let found =
                seek_with_mode(&lines, std::slice::from_ref(context), position, false, mode)
                    .ok_or_else(|| {
                        failure(
                            format!("Failed to find context in {path}:"),
                            &lines,
                            std::slice::from_ref(context),
                            position,
                            mode,
                            None,
                        )
                    })?;
            context_index = Some(found);
            position = found + 1;
        }
        if chunk.old_lines.is_empty() {
            let at = if mode == FileUpdateMode::NormalizeToLf
                && lines.last().is_some_and(String::is_empty)
            {
                lines.len() - 1
            } else {
                lines.len()
            };
            replacements.push((at, 0, chunk.new_lines.clone()));
            continue;
        }
        let mut old = chunk.old_lines.as_slice();
        let mut new = chunk.new_lines.as_slice();
        let mut found = seek_with_mode(&lines, old, position, chunk.is_end_of_file, mode);
        if found.is_none() && old.last().is_some_and(String::is_empty) {
            old = &old[..old.len() - 1];
            if new.last().is_some_and(String::is_empty) {
                new = &new[..new.len() - 1];
            }
            found = seek_with_mode(&lines, old, position, chunk.is_end_of_file, mode);
        }
        let at = found.ok_or_else(|| {
            failure(
                format!("Failed to find expected lines in {path}:"),
                &lines,
                old,
                position,
                mode,
                context_index,
            )
        })?;
        if mode == FileUpdateMode::NormalizeToLf {
            replacements.push((at, old.len(), new.to_vec()));
        } else {
            let (mut a, mut b) = (0, 0);
            for &(x, y) in &chunk.context_line_indices {
                if x >= old.len() || y >= new.len() {
                    break;
                }
                if a != x || b != y {
                    replacements.push((at + a, x - a, new[b..y].to_vec()));
                }
                a = x + 1;
                b = y + 1;
            }
            if a != old.len() || b != new.len() {
                replacements.push((at + a, old.len() - a, new[b..].to_vec()));
            }
        }
        position = at + old.len();
    }
    replacements.sort_by_key(|r| r.0);
    let result = if mode == FileUpdateMode::NormalizeToLf {
        let mut output = lines;
        for (at, len, new) in replacements.into_iter().rev() {
            let at = at.min(output.len());
            let end = at.saturating_add(len).min(output.len());
            output.splice(at..end, new);
        }
        if output.last().is_none_or(|line| !line.is_empty()) {
            output.push(String::new());
        }
        output.join("\n")
    } else {
        text::apply(contents, &replacements)
    };
    Ok(format!("{mark}{result}"))
}
