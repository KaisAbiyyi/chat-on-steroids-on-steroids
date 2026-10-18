//! Fill recorded DIL table rows without rewriting other Markdown.
use crate::content::{TextError, plain_text_of_html, re};
use cos_core::config::js_trim;
use serde_json::Value;

fn cell_text(value: &str) -> String {
    let bare = value.replace("**", "");
    js_trim(&re!(&format!(r"[{}]+", crate::title::JS_SPACE)).replace_all(&bare, " ")).to_owned()
}
fn markdown_cells(line: &str) -> Vec<String> {
    let line = js_trim(line);
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    let mut cells = Vec::new();
    let mut start = 0;
    let mut previous = None;
    for (at, ch) in line.char_indices() {
        if ch == '|' && previous != Some('\\') {
            cells.push(cell_text(&line[start..at].replace("\\|", "|")));
            start = at + 1;
        }
        previous = Some(ch);
    }
    cells.push(cell_text(&line[start..].replace("\\|", "|")));
    cells
}
fn drawn_tables(html: &str) -> Result<Vec<Vec<Vec<String>>>, TextError> {
    let mut tables = Vec::new();
    let table_pattern = re!(r"(?is:<table(?-u:\b).*?</table>)");
    let row_pattern = re!(r"(?is:<tr(?-u:\b).*?</tr>)");
    let cell_pattern = re!(r"(?is:<t[dh](?-u:\b)[^>]*>(.*?)</t[dh]>)");
    for marked in html.split("data-d-component=\"table\"").skip(1) {
        let Some(table) = table_pattern.find(marked) else {
            continue;
        };
        let mut rows = Vec::new();
        for row in row_pattern.find_iter(table.as_str()) {
            let mut cells = Vec::new();
            for cell in cell_pattern.captures_iter(row.as_str()) {
                cells.push(cell_text(&plain_text_of_html(&cell[1])?));
            }
            rows.push(cells);
        }
        tables.push(rows);
    }
    Ok(tables)
}
pub fn completed_dil_text(text: &str, capture: Option<&Value>) -> Result<String, TextError> {
    let Some(capture) = capture else {
        return Ok(text.into());
    };
    let Some(html) = capture["text"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(text.into());
    };
    if capture["truncated"] == true || !html.contains("data-d-component") {
        return Ok(text.into());
    }
    let tables = drawn_tables(html)?;
    let mut buttons = Vec::new();
    for capture in re!(r#"(?is:<button(?-u:\b)[^>]*data-d-component="button"[^>]*>(.*?)</button>)"#)
        .captures_iter(html)
    {
        let button = cell_text(&plain_text_of_html(&capture[1])?);
        if !button.is_empty() && !buttons.contains(&button) {
            buttons.push(button);
        }
    }
    if tables.is_empty() && buttons.is_empty() {
        return Ok(text.into());
    }
    let lines = text.split('\n').collect::<Vec<_>>();
    let mut output: Vec<String> = Vec::new();
    let mut at = 0;
    let button_label = re!(r"^\[([^\]]+)\]$");
    let separator = re!(&format!(
        r"^[{0}]*\|?[{0}]*:?-{{3,}}:?[{0}]*(\|[{0}]*:?-{{3,}}:?[{0}]*)*\|?[{0}]*$",
        crate::title::JS_SPACE
    ));
    while let Some(line) = lines.get(at) {
        if let Some(label) = button_label.captures(js_trim(line))
            && buttons.contains(&cell_text(&label[1]))
        {
            if output.last().is_some_and(|line| js_trim(line).is_empty())
                && lines
                    .get(at + 1)
                    .is_some_and(|line| js_trim(line).is_empty())
            {
                at += 1;
            }
            at += 1;
            continue;
        }
        output.push((*line).into());
        if js_trim(line).starts_with('|') && separator.is_match(lines.get(at + 1).unwrap_or(&"")) {
            let header = markdown_cells(line);
            let mut body = 0;
            while lines
                .get(at + 2 + body)
                .is_some_and(|line| js_trim(line).starts_with('|'))
            {
                body += 1;
            }
            if let Some(rows) = tables
                .iter()
                .find(|rows| rows.len().saturating_sub(1) > body && rows[0] == header)
            {
                output.push(lines[at + 1].into());
                for row in rows.iter().skip(1) {
                    let cells = row
                        .iter()
                        .map(|cell| cell.replace('\\', "\\\\").replace('|', "\\|"))
                        .collect::<Vec<_>>();
                    output.push(format!("| {} |", cells.join(" | ")));
                }
                at += 2 + body;
                continue;
            }
        }
        at += 1;
    }
    Ok(output.join("\n"))
}
