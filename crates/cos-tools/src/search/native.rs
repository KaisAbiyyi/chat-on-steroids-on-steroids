use super::engine::{Backend, Entry, Kind, Metadata};
use crate::files::{self, handle_path};
use cap_std::fs::{Dir, ReadDir};
use cos_core::{config::Root, persistence::invalid, sandbox};
use std::{
    borrow::Cow,
    io::{self, Read},
    path::PathBuf,
};

pub struct Native<'a> {
    roots: Cow<'a, [Root]>,
}
impl<'a> Native<'a> {
    pub fn new(roots: &'a [Root]) -> Self {
        Self {
            roots: Cow::Borrowed(roots),
        }
    }
}
struct Folder {
    directory: Dir,
    entries: ReadDir,
    approved: PathBuf,
}
impl Iterator for Folder {
    type Item = io::Result<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        let check = || {
            let actual = handle_path(&self.directory.try_clone()?.into_std_file())?;
            if !sandbox::contained(&self.approved, &actual) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Search directory escaped approved root",
                ));
            }
            Ok(())
        };
        if let Err(error) = check() {
            return Some(Err(error));
        }
        let entry = self.entries.next()?;
        Some((|| {
            let entry = entry?;
            check()?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| invalid("Search filename is not representable as UTF-8"))?
                .to_owned();
            let metadata = self.directory.symlink_metadata(&name)?;
            #[cfg(windows)]
            let link = {
                use cap_std::fs::MetadataExt;
                metadata.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let link = metadata.file_type().is_symlink();
            let kind = if link {
                Kind::Other
            } else if metadata.is_dir() {
                Kind::Directory
            } else if metadata.is_file() {
                Kind::File
            } else {
                Kind::Other
            };
            check()?;
            Ok(Entry { name, kind })
        })())
    }
}
impl Backend for Native<'_> {
    fn fork_reader(&self) -> Option<Box<dyn Backend>> {
        Some(Box::new(Native {
            roots: Cow::Owned(self.roots.to_vec()),
        }))
    }
    fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
        let location = files::locate(&self.roots, path, None, false)?;
        let meta = location.root.metadata(&location.relative)?;
        Ok(Metadata {
            kind: if meta.is_file() {
                Kind::File
            } else if meta.is_dir() {
                Kind::Directory
            } else {
                Kind::Other
            },
            bytes: meta.len(),
        })
    }
    fn open_folder(
        &mut self,
        path: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
        let location = files::locate(&self.roots, path, None, false)?;
        let directory = location.root.open_dir(&location.relative)?;
        let entries = directory.entries()?;
        Ok(Box::new(Folder {
            directory,
            entries,
            approved: location.approved,
        }))
    }
    fn read_file(
        &mut self,
        path: &str,
        max: usize,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        authorize()?;
        let location = files::locate(&self.roots, path, None, false)?;
        let mut file = location.open_handle()?;
        let mut bytes = vec![];
        let mut buffer = [0; 65536];
        loop {
            authorize()?;
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if bytes.len() + count > max {
                return Err(invalid("Search file grew beyond the content byte limit"));
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        if !sandbox::contained(&location.approved, &handle_path(&file)?) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Opened search file escaped approved root",
            ));
        }
        authorize()?;
        Ok(bytes)
    }
}
