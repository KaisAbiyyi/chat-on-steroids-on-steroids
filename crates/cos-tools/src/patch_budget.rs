//! Conservative transaction accounting, including line-vector workspace and snapshots.
use crate::patch::{Chunk, Hunk};
use cos_core::persistence::invalid;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};
const LIMIT: usize = 256 * 1024 * 1024;
const FILES: usize = 128;
fn cost(bytes: usize, lines: usize) -> usize {
    bytes
        .saturating_mul(8)
        .saturating_add(lines.saturating_mul(192))
}
fn breaks(bytes: &[u8]) -> usize {
    bytes.iter().filter(|b| matches!(b, b'\r' | b'\n')).count() + 1
}
fn refused() -> io::Error {
    invalid(
        "PATCH_BUDGET_EXCEEDED: split the patch into smaller transactions (128 files, 256 MiB estimated workspace)",
    )
}
pub(crate) fn check_source(source: &str) -> io::Result<()> {
    check_source_counts(source.len(), breaks(source.as_bytes()))
}
pub(crate) fn check_source_counts(bytes: usize, breaks: usize) -> io::Result<()> {
    if cost(bytes, breaks.saturating_mul(3)) > LIMIT {
        return Err(refused());
    }
    Ok(())
}
pub(crate) struct Budget {
    used: usize,
    files: BTreeMap<String, usize>,
}
impl Budget {
    pub fn new(hunks: &[Hunk]) -> io::Result<Self> {
        let mut names = BTreeSet::new();
        let mut used: usize = 0;
        for hunk in hunks {
            match hunk {
                Hunk::AddFile { path, contents } => {
                    names.insert(path.as_str());
                    used = used.saturating_add(cost(contents.len(), breaks(contents.as_bytes())));
                }
                Hunk::DeleteFile { path } => {
                    names.insert(path.as_str());
                }
                Hunk::UpdateFile {
                    path,
                    move_path,
                    chunks,
                } => {
                    names.insert(path.as_str());
                    if let Some(path) = move_path {
                        names.insert(path.as_str());
                    }
                    for chunk in chunks {
                        for line in chunk.old_lines.iter().chain(&chunk.new_lines) {
                            used = used.saturating_add(cost(line.len(), 1));
                        }
                    }
                }
            }
        }
        if names.len() > FILES || used > LIMIT {
            return Err(refused());
        }
        Ok(Self {
            used,
            files: BTreeMap::new(),
        })
    }
    pub fn read_limit(&self, key: &str) -> io::Result<usize> {
        if !self.files.contains_key(key) && self.files.len() >= FILES {
            return Err(refused());
        }
        Ok((LIMIT - self.used) / 8)
    }
    fn reserve(&mut self, key: &str, bytes: usize, lines: usize) -> io::Result<()> {
        self.read_limit(key)?;
        let amount = cost(bytes, lines);
        let previous = self.files.get(key).copied().unwrap_or(0);
        let next = self.used.saturating_add(amount.saturating_sub(previous));
        if next > LIMIT {
            return Err(refused());
        }
        self.used = next;
        self.files.insert(key.into(), previous.max(amount));
        Ok(())
    }
    pub fn bytes(&mut self, key: &str, bytes: &[u8]) -> io::Result<()> {
        self.reserve(key, bytes.len(), breaks(bytes))
    }
    pub fn update(&mut self, key: &str, old: &[u8], chunks: &[Chunk]) -> io::Result<()> {
        let mut size = old.len().saturating_add(2);
        let mut lines = breaks(old).saturating_add(1);
        for chunk in chunks {
            for line in &chunk.new_lines {
                size = size.saturating_add(line.len()).saturating_add(2);
                lines += 1;
            }
        }
        self.reserve(key, size, lines)
    }
}
