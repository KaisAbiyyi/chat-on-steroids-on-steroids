use super::{DEFAULT_EXCLUDES, IncludeMatcher, Options};
use crate::files::is_excluded_folder;
use cos_core::persistence::invalid;
use serde::Serialize;
use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};

pub const MAX_CONTENT_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_FILES: usize = 40_000;
const MAX_DIRECTORIES: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Directory,
    Other,
}
pub struct Metadata {
    pub kind: Kind,
    pub bytes: u64,
}
pub struct Entry {
    pub name: String,
    pub kind: Kind,
}
/// Each backend retains its native entry order and keeps links opaque during traversal.
/// All methods are beneath the caller's approved-root and runtime trust boundary.
pub trait Backend: Send {
    /// Independent readers allow native content jobs to overlap. A protocol owner
    /// with one mutable request stream retains serialization through the shared adapter.
    fn fork_reader(&self) -> Option<Box<dyn Backend>> {
        None
    }
    fn metadata(&mut self, path: &str) -> io::Result<Metadata>;
    fn open_folder(
        &mut self,
        path: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>>;
    fn read_file(
        &mut self,
        path: &str,
        max: usize,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>>;
}
#[derive(Serialize, Debug)]
pub struct Hit {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum StopReason {
    Limit,
    Time,
    Files,
    Size,
}
impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Limit => "limit",
            Self::Time => "time",
            Self::Files => "files",
            Self::Size => "size",
        }
    }
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub hits: Vec<Hit>,
    pub files_scanned: usize,
    pub truncated: bool,
    pub stopped_because: Option<StopReason>,
    pub elapsed_ms: u64,
    #[serde(skip)]
    pub skipped: usize,
}
impl Outcome {
    pub(super) fn stop(&mut self, reason: StopReason) {
        self.truncated = true;
        self.stopped_because = Some(reason);
    }
    fn budget(&mut self, deadline: Instant) -> bool {
        if Instant::now() > deadline {
            self.stop(StopReason::Time);
            true
        } else if self.files_scanned >= MAX_FILES {
            self.stop(StopReason::Files);
            true
        } else {
            false
        }
    }
}
pub fn search(
    backend: &mut dyn Backend,
    base: &str,
    options: &Options<'_>,
    whole_tool_deadline: Instant,
    authorize: &dyn Fn() -> io::Result<()>,
) -> io::Result<Outcome> {
    authorize()?;
    if options.limit == 0 || options.limit > 500 {
        return Err(invalid("limit must be between 1 and 500"));
    }
    if options.regex && !options.content {
        return Err(invalid("regex=true is only valid with mode=content"));
    }
    let started = Instant::now();
    let deadline = whole_tool_deadline.min(started + Duration::from_secs(10));
    let mut outcome = Outcome {
        hits: vec![],
        files_scanned: 0,
        truncated: false,
        stopped_because: None,
        elapsed_ms: 0,
        skipped: 0,
    };
    let include = options
        .glob
        .filter(|text| !text.is_empty())
        .map(|glob| IncludeMatcher::new(glob, options.case_sensitive))
        .transpose()?;
    let needle = if options.case_sensitive {
        options.query.into()
    } else {
        options.query.to_lowercase()
    };
    let matching_name = |name: &str| {
        if options.case_sensitive {
            name.contains(&needle)
        } else {
            name.to_lowercase().contains(&needle)
        }
    };
    let metadata = backend.metadata(base)?;
    if metadata.kind == Kind::File {
        let name = base.rsplit('/').next().unwrap_or(base);
        outcome.files_scanned = 1;
        if include.as_ref().is_none_or(|matcher| matcher.matches(name)) {
            if !options.content {
                if matching_name(name) {
                    outcome.hits.push(Hit {
                        path: base.into(),
                        line: None,
                        text: None,
                    });
                }
            } else if metadata.bytes > MAX_CONTENT_FILE_BYTES as u64 {
                outcome.files_scanned = 0;
                outcome.stop(StopReason::Size);
            } else {
                let found = scan_file(backend, base, options, authorize, &mut outcome.skipped)?;
                let exceeded = found.len() > options.limit;
                outcome.hits = found.into_iter().take(options.limit).collect();
                if exceeded {
                    outcome.stop(StopReason::Limit);
                }
            }
        }
    } else if metadata.kind == Kind::Directory {
        let excludes = options
            .exclude
            .map(<[String]>::to_vec)
            .unwrap_or_else(|| DEFAULT_EXCLUDES.iter().map(|s| (*s).into()).collect());
        let mut queue = VecDeque::from([(base.trim_end_matches('/').to_owned(), String::new())]);
        let mut directories = 0;
        let mut queued_overflow = false;
        let mut candidates = vec![];
        while let Some((path, relative)) = queue.pop_front() {
            authorize()?;
            if outcome.budget(deadline) {
                break;
            }
            if directories >= MAX_DIRECTORIES {
                outcome.stop(StopReason::Files);
                break;
            }
            directories += 1;
            let entries = match backend.open_folder(&path) {
                Ok(entries) => entries,
                // A folder that cannot be opened contributes no entries, as in the source.
                Err(_) => {
                    authorize()?;
                    continue;
                }
            };
            for entry in entries {
                authorize()?;
                if outcome.budget(deadline) {
                    break;
                }
                let entry = entry?;
                let child_rel = if relative.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{relative}/{}", entry.name)
                };
                let child = format!("{path}/{}", entry.name);
                match entry.kind {
                    Kind::Directory if !is_excluded_folder(&entry.name, &excludes) => {
                        // Only the first 10,000 enqueued directories can ever be visited.
                        // Bound the queue without truncating files in the current directory.
                        if directories + queue.len() < MAX_DIRECTORIES {
                            queue.push_back((child, child_rel));
                        } else {
                            queued_overflow = true;
                        }
                    }
                    Kind::File => {
                        outcome.files_scanned += 1;
                        if include
                            .as_ref()
                            .is_some_and(|matcher| !matcher.matches(&child_rel))
                        {
                            continue;
                        }
                        if options.content {
                            candidates.push(child);
                        } else if matching_name(&entry.name) {
                            outcome.hits.push(Hit {
                                path: child,
                                line: None,
                                text: None,
                            });
                            if outcome.hits.len() >= options.limit {
                                outcome.stop(StopReason::Limit);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if outcome.stopped_because == Some(StopReason::Limit) {
                break;
            }
        }
        if queued_overflow && outcome.stopped_because.is_none() {
            outcome.stop(StopReason::Files);
        }
        if options.content {
            super::scheduling::scan(
                backend,
                candidates,
                options,
                deadline,
                authorize,
                &mut outcome,
            )?;
        }
    } else {
        return Err(invalid(format!("{base} is not a regular file or folder")));
    }
    authorize()?;
    outcome.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(outcome)
}

pub(super) fn scan_file(
    backend: &mut dyn Backend,
    path: &str,
    options: &Options<'_>,
    authorize: &dyn Fn() -> io::Result<()>,
    skipped: &mut usize,
) -> io::Result<Vec<Hit>> {
    authorize()?;
    let metadata = match backend.metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            authorize()?;
            *skipped += 1;
            return Ok(vec![]);
        }
    };
    if metadata.kind != Kind::File
        || metadata.bytes == 0
        || metadata.bytes > MAX_CONTENT_FILE_BYTES as u64
    {
        *skipped += 1;
        return Ok(vec![]);
    }
    let bytes = match backend.read_file(path, MAX_CONTENT_FILE_BYTES, authorize) {
        Ok(bytes) => bytes,
        Err(_) => {
            authorize()?;
            *skipped += 1;
            return Ok(vec![]);
        }
    };
    if crate::project_files::sniff(&bytes[..bytes.len().min(8192)]) {
        *skipped += 1;
        return Ok(vec![]);
    }
    // The source uses TextDecoder's replacement behavior, not strict editor decoding.
    let text = decode_search(&bytes);
    let expression = if options.regex {
        Some(
            regex::RegexBuilder::new(options.query)
                .case_insensitive(!options.case_sensitive)
                .size_limit(1024 * 1024)
                .build()
                .map_err(|error| invalid(format!("Invalid regex: {error}")))?,
        )
    } else {
        None
    };
    let needle = if options.case_sensitive {
        options.query.into()
    } else {
        options.query.to_lowercase()
    };
    let mut found = vec![];
    for (index, line) in text.split_terminator('\n').enumerate() {
        if index % 256 == 0 {
            authorize()?;
        }
        let line = line.strip_suffix('\r').unwrap_or(line);
        let matched = if let Some(regex) = &expression {
            regex.is_match(line)
        } else if options.case_sensitive {
            line.contains(&needle)
        } else {
            line.to_lowercase().contains(&needle)
        };
        if matched {
            let trimmed = cos_core::config::js_trim(line);
            let text = if trimmed.encode_utf16().count() > 300 {
                let units: Vec<_> = trimmed.encode_utf16().take(300).collect();
                // A split surrogate needs the workspace-wide lossless JSON string owner.
                let prefix = String::from_utf16(&units).map_err(|_| io::Error::new(io::ErrorKind::Unsupported, "Search result crosses an unpaired UTF-16 boundary; lossless JSON string support is not implemented"))?;
                format!("{prefix}…")
            } else {
                trimmed.into()
            };
            found.push(Hit {
                path: path.into(),
                line: Some(index + 1),
                text: Some(text),
            });
            // Only one extra hit is needed to distinguish an explicit-file limit.
            if found.len() > options.limit {
                break;
            }
        }
    }
    authorize()?;
    Ok(found)
}
fn decode_search(bytes: &[u8]) -> String {
    if bytes.starts_with(&[255, 254]) || bytes.starts_with(&[254, 255]) {
        let little = bytes[0] == 255;
        let mut units: Vec<_> = bytes[2..]
            .chunks_exact(2)
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        if !(bytes.len() - 2).is_multiple_of(2) {
            units.push(0xfffd);
        }
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[239, 187, 191]).unwrap_or(bytes)).into()
    }
}
