use crate::kernel::Kernel;
use cos_core::{config::Root, persistence::invalid};
use serde_json::{Map, Value, json};
use std::{io, path::Path};
struct Section {
    text: String,
    image: Option<Value>,
}
fn bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024. * 1024.))
    } else {
        format!("{:.2} GB", n as f64 / (1024. * 1024. * 1024.))
    }
}
fn range(input: &str) -> (String, Option<(usize, Option<usize>)>) {
    let re = regex::Regex::new(r"^(.*[^\\/:]):(\d{1,9})(?:-(\d{1,9}))?$").unwrap();
    if let Some(c) = re.captures(input) {
        let start = c[2].parse::<usize>().unwrap_or(0);
        let end = c.get(3).and_then(|c| c.as_str().parse::<usize>().ok());
        if start >= 1 && end.is_none_or(|e| e >= start) {
            return (c[1].into(), Some((start, end)));
        }
    }
    (input.into(), None)
}
impl Kernel {
    pub(crate) fn read_tool(
        &self,
        roots: &[Root],
        args: &Value,
        caps: &Map<String, Value>,
        request: &str,
    ) -> io::Result<Value> {
        if !["read", "browse", "metadata"]
            .iter()
            .any(|k| caps[*k] == true)
        {
            return Err(invalid(
                "TOOL_DISABLED: read is disabled by the current Chat On Steroids permissions.",
            ));
        }
        let mut targets = vec![];
        let mut sections = vec![];
        for input in args["paths"].as_array().unwrap() {
            let (path, range) = range(input.as_str().unwrap());
            if path.contains(['*', '?']) {
                match self.files.glob(roots, &path) {
                    Ok(paths) => {
                        if paths.is_empty() {
                            sections.push(format!("(no paths matched {path})"));
                        }
                        targets.extend(paths.into_iter().map(|p| (p, range)));
                    }
                    Err(e) => sections.push(format!(
                        "--- {path} — ERROR ---\n{}",
                        cos_core::logging::redact_credentials(&e.to_string())
                    )),
                }
            } else {
                targets.push((path, range));
            }
            if targets.len() >= 40 {
                targets.truncate(40);
                sections.push("(stopped expanding at 40 paths)".into());
                break;
            }
        }
        let mut remaining = 512 * 1024;
        let mut images = vec![];
        let mut image_bytes = 0;
        let mut successes = 0;
        let mut failures = 0;
        for (path, range) in targets {
            if remaining == 0 {
                sections
                    .push("(output cap reached; read the remaining paths in another call)".into());
                break;
            }
            let (start, end) = range.unwrap_or((
                args["start_line"].as_u64().unwrap_or(1) as usize,
                args["end_line"].as_u64().map(|v| v as usize),
            ));
            let result = self.read_one(
                roots,
                &path,
                (start, end.unwrap_or(100_000_000)),
                remaining.min(args["max_bytes"].as_u64().unwrap_or(256 * 1024) as usize),
                caps,
                if images.len() < 4 {
                    12 * 1024 * 1024 - image_bytes
                } else {
                    0
                },
            );
            self.cancel.check()?;
            self.admit(request)?;
            if self.roots()? != roots
                || &cos_core::config::effective_capabilities(
                    &self.config.get(),
                    if cfg!(windows) {
                        "win32"
                    } else if cfg!(target_os = "macos") {
                        "darwin"
                    } else {
                        "linux"
                    },
                ) != caps
            {
                return Err(invalid("Workspace or permission changed during read"));
            }
            match result {
                Ok(section) => {
                    successes += 1;
                    remaining = remaining.saturating_sub(section.text.len());
                    sections.push(section.text);
                    if let Some(image) = section.image {
                        image_bytes += image["data"].as_str().unwrap().len();
                        images.push(image);
                    }
                }
                Err(e) => {
                    failures += 1;
                    sections.push(format!(
                        "--- {path} — ERROR ---\n{}",
                        cos_core::logging::redact_credentials(&e.to_string())
                    ));
                }
            }
        }
        let mut content = vec![json!({"type":"text","text":sections.join("\n\n")})];
        content.extend(images);
        let mut result = json!({"content":content});
        if successes == 0 && failures > 0 {
            result["isError"] = json!(true);
        }
        Ok(result)
    }
    fn read_one(
        &self,
        roots: &[Root],
        path: &str,
        range: (usize, usize),
        max_bytes: usize,
        caps: &Map<String, Value>,
        image_budget: usize,
    ) -> io::Result<Section> {
        let (start, end) = range;
        if let Some((root, tail)) = self.remote_root(roots, path) {
            return self.with_remote(root, |r| {
                let real = r.resolve(&tail, false)?;
                let meta = r.sftp.stat(&real)?;
                if meta["isDirectory"] == true {
                    if caps["browse"] != true {
                        return Err(invalid("Folder browsing is disabled"));
                    }
                    let list = r.sftp.list(&real)?;
                    let clipped = list.len() > 200;
                    let rows = list
                        .iter()
                        .take(200)
                        .map(|e| {
                            format!(
                                "{}{}",
                                e["name"].as_str().unwrap_or(""),
                                if e["isDirectory"] == true { "/" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Ok(Section {
                        text: format!(
                            "--- {path} ---\n{rows}{}",
                            if clipped {
                                "\n(directory entry cap reached)"
                            } else {
                                ""
                            }
                        ),
                        image: None,
                    });
                }
                if caps["read"] != true {
                    return Ok(Section {
                        text: format!(
                            "--- {path} ---\nsize: {}\nFile content reading is disabled.",
                            bytes(meta["size"].as_u64().unwrap_or(0))
                        ),
                        image: None,
                    });
                }
                if Path::new(path).extension().is_some_and(|e| {
                    ["png", "jpg", "jpeg", "gif", "webp"]
                        .iter()
                        .any(|v| e.to_string_lossy().eq_ignore_ascii_case(v))
                }) {
                    let raw = r.sftp.read(&real, cos_tools::image::MAX_BYTES)?;
                    let image = cos_tools::image::content(&raw)?;
                    if image["data"].as_str().unwrap().len() > image_budget {
                        return Err(invalid("Read image output cap reached"));
                    }
                    return Ok(Section {
                        text: format!("--- {path} — {} ---", bytes(raw.len() as u64)),
                        image: Some(image),
                    });
                }
                match r.sftp.preview(&real, start, end, max_bytes) {
                    Ok(read) => Ok(preview_section(path, &read, max_bytes)),
                    Err(e) if e.to_string().contains("Binary file") => Ok(Section {
                        text: format!(
                            "--- {path} ---\nBinary file, so its bytes are not returned."
                        ),
                        image: None,
                    }),
                    Err(e) => Err(e),
                }
            });
        }
        let l = cos_tools::files::locate(roots, path, None, false)?;
        let metadata = l.root.metadata(&l.relative)?;
        if metadata.is_dir() {
            if caps["browse"] != true {
                return Err(invalid("Folder browsing is disabled"));
            }
            let list = self.files.list(roots, path, 200)?;
            let rows = list["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    format!(
                        "{}{}",
                        e["name"].as_str().unwrap(),
                        if e["directory"] == true { "/" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(Section {
                text: format!(
                    "--- {} ---\n{}{}",
                    l.virtual_path,
                    rows,
                    if list["truncated"] == true {
                        "\n(directory entry cap reached)"
                    } else {
                        ""
                    }
                ),
                image: None,
            });
        }
        if caps["read"] != true {
            return Ok(Section {
                text: format!(
                    "--- {} ---\nsize: {}\nFile content reading is disabled.",
                    l.virtual_path,
                    bytes(metadata.len())
                ),
                image: None,
            });
        }
        if Path::new(path).extension().is_some_and(|e| {
            ["png", "jpg", "jpeg", "gif", "webp"]
                .iter()
                .any(|v| e.to_string_lossy().eq_ignore_ascii_case(v))
        }) {
            if image_budget == 0 {
                return Err(invalid(
                    "Read image output cap reached; read remaining images in another call or use view_image.",
                ));
            }
            let raw = l.open(cos_tools::image::MAX_BYTES as u64)?;
            let image = cos_tools::image::content(&raw)?;
            if image["data"].as_str().unwrap().len() > image_budget {
                return Err(invalid(
                    "Read image output cap reached (12.0 MB base64 per call); read remaining images in another call or use view_image.",
                ));
            }
            return Ok(Section {
                text: format!(
                    "--- {} — {} {} ---",
                    l.virtual_path,
                    bytes(raw.len() as u64),
                    image["mimeType"].as_str().unwrap()
                ),
                image: Some(image),
            });
        }
        let read = match self.files.read(roots, path, start, end, max_bytes) {
            Ok(r) => r,
            Err(e) if e.to_string().contains("Binary file") => {
                return Ok(Section {
                    text: format!(
                        "--- {} ---\nsize: {}\nBinary file, so its bytes are not returned. Use exec_command if you need to inspect or convert it.",
                        l.virtual_path,
                        bytes(metadata.len())
                    ),
                    image: None,
                });
            }
            Err(e) => return Err(e),
        };
        Ok(preview_section(&l.virtual_path, &read, max_bytes))
    }
}
fn preview_section(path: &str, read: &Value, max_bytes: usize) -> Section {
    let text = read["text"].as_str().unwrap();
    let first = read["firstLine"].as_u64().unwrap();
    let mut visible = first.saturating_sub(1);
    let mut numbered = vec![];
    let mut used = 0;
    if read["lastLine"].as_u64().unwrap() >= first {
        for (i, line) in text.split('\n').enumerate() {
            let row = format!("{}\t{line}", first + i as u64);
            if used + row.len() + 1 > max_bytes {
                break;
            }
            used += row.len() + 1;
            visible = first + i as u64;
            numbered.push(row);
        }
    }
    let range = if visible < first {
        "no lines in that range".into()
    } else if let Some(total) = read["totalLines"].as_u64() {
        format!("lines {first}-{visible} of {total}")
    } else {
        format!("lines {first}-{visible}")
    };
    let note = if read["truncated"] == true || visible < read["lastLine"].as_u64().unwrap() {
        format!(
            "\n(output cap reached; continue from line {} or raise max_bytes up to 524288)",
            visible + 1
        )
    } else if read["more"] == true {
        format!("\n(more lines follow; continue from line {})", visible + 1)
    } else {
        String::new()
    };
    Section {
        text: format!(
            "--- {} — {} ---\nsize: {}\n{}{}",
            path,
            range,
            bytes(read["size"].as_u64().unwrap_or(0)),
            numbered.join("\n"),
            note
        ),
        image: None,
    }
}
