use super::engine::{Backend, Entry, Kind, Metadata};
use crate::remote::RemoteFiles;
use cos_core::persistence::invalid;
use serde_json::Value;
use std::io;

/// The host must admit its explicit lifetime SSH path trust before constructing this adapter.
pub struct Server<'a> {
    remote: &'a mut RemoteFiles,
    virtual_root: String,
}
impl<'a> Server<'a> {
    pub fn new(remote: &'a mut RemoteFiles, virtual_root: String) -> Self {
        Self {
            remote,
            virtual_root,
        }
    }
    fn real(&mut self, path: &str) -> io::Result<String> {
        let relative = if path == self.virtual_root {
            ""
        } else {
            path.strip_prefix(&format!("{}/", self.virtual_root))
                .ok_or_else(|| invalid("Search path escaped its server root"))?
        };
        self.remote.resolve(relative, false)
    }
}
fn metadata(value: &Value) -> Metadata {
    Metadata {
        kind: if value["isSymbolicLink"] == true {
            Kind::Other
        } else if value["isFile"] == true {
            Kind::File
        } else if value["isDirectory"] == true {
            Kind::Directory
        } else {
            Kind::Other
        },
        bytes: value["size"].as_u64().unwrap_or(0),
    }
}
impl Backend for Server<'_> {
    fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
        let real = self.real(path)?;
        Ok(metadata(&self.remote.sftp.stat(&real)?))
    }
    fn open_folder(
        &mut self,
        path: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
        let real = self.real(path)?;
        let entries = self.remote.sftp.list_native(&real)?;
        Ok(Box::new(entries.into_iter().map(|value| {
            let name = value["name"]
                .as_str()
                .ok_or_else(|| invalid("Invalid server directory entry"))?
                .to_owned();
            if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
                return Err(invalid("Invalid server search entry name"));
            }
            Ok(Entry {
                kind: metadata(&value).kind,
                name,
            })
        })))
    }
    fn read_file(
        &mut self,
        path: &str,
        max: usize,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        authorize()?;
        let real = self.real(path)?;
        self.remote.sftp.read_stream(&real, |reader, info| {
            if info["size"].as_u64().unwrap_or(u64::MAX) > max as u64 {
                return Err(invalid("Search file exceeds the content byte limit"));
            }
            let mut bytes = vec![];
            let mut buffer = [0; 65536];
            loop {
                authorize()?;
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                if bytes.len() + count > max {
                    return Err(invalid("Search file grew beyond the content byte limit"));
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            authorize()?;
            Ok(bytes)
        })
    }
}
