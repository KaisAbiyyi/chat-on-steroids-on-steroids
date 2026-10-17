use super::*;
use crate::remote::RemoteFiles;

struct Authorized<'a> {
    reader: &'a mut dyn Read,
    check: &'a dyn Fn() -> io::Result<()>,
}
impl Read for Authorized<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        (self.check)()?;
        self.reader.read(out)
    }
}

impl ProjectFiles {
    pub fn server_snapshot(
        remote: &mut RemoteFiles,
        input: &str,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<ServerSnapshot> {
        const MAX_ATTACHMENT_BYTES: u64 = 512 * 1024 * 1024;
        authorize()?;
        let path = relative(input)?;
        if path.is_empty() {
            return Err(invalid("Choose a regular file"));
        }
        let name = leaf(path.rsplit('/').next().unwrap())?;
        let real = remote.resolve(&path, false)?;
        let directory = tempfile::Builder::new().prefix("cos-attach-").tempdir()?;
        let parent = Dir::open_ambient_dir(directory.path(), cap_std::ambient_authority())?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut output = parent.open_with(name, &options)?;
        remote.sftp.read_stream(&real, |reader, meta| {
            if meta["size"].as_u64().unwrap_or(u64::MAX) > MAX_ATTACHMENT_BYTES {
                return Err(invalid("Each attachment must be 512 MB or smaller"));
            }
            let mut bytes = 0u64;
            let mut buffer = vec![0u8; 65536];
            loop {
                authorize()?;
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                bytes += count as u64;
                if bytes > MAX_ATTACHMENT_BYTES {
                    return Err(invalid("Each attachment must be 512 MB or smaller"));
                }
                output.write_all(&buffer[..count])?;
            }
            output.sync_all()
        })?;
        drop(output);
        drop(parent);
        authorize()?;
        if remote.resolve(&path, false)? != real {
            return Err(invalid(
                "The project file changed location. Reload it before continuing.",
            ));
        }
        Ok(ServerSnapshot {
            file: directory.path().join(name),
            _directory: directory,
        })
    }
    /// Low-level server adapter. The host must enforce explicit runtime path trust.
    pub fn server_read(
        remote: &mut RemoteFiles,
        id: &str,
        name: &str,
        input: &str,
        preview: bool,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Value> {
        authorize()?;
        let path = relative(input)?;
        let real = remote.resolve(&path, false)?;
        let result = if preview {
            if path.is_empty() {
                return Err(invalid("Choose a regular file"));
            }
            remote.sftp.read_stream(&real, |reader, meta| {
                let bytes = meta["size"]
                    .as_u64()
                    .ok_or_else(|| invalid("Invalid remote file size"))?;
                let modified = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                    meta["mtimeMs"]
                        .as_i64()
                        .ok_or_else(|| invalid("Invalid remote file timestamp"))?,
                )
                .ok_or_else(|| invalid("Invalid remote file timestamp"))?
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                let mut result = json!({
                    "projectId": id, "projectName": name, "path": path,
                    "name": path.rsplit('/').next().unwrap(), "bytes": bytes,
                    "modifiedAt": modified, "binary": true, "text": null, "truncated": false
                });
                if image_extension(&path) || path.to_ascii_lowercase().ends_with(".pdf") {
                    result["note"] = json!(
                        "Previews of images and PDFs in a server folder are not available yet."
                    );
                    return Ok(result);
                }
                let mut reader = Authorized {
                    reader,
                    check: authorize,
                };
                let mut head = vec![];
                Read::by_ref(&mut reader)
                    .take(8192)
                    .read_to_end(&mut head)?;
                if sniff(&head) {
                    result["note"] = json!("Binary file, so there is no text preview.");
                    return Ok(result);
                }
                result["binary"] = json!(false);
                match files::preview(
                    io::Cursor::new(head).chain(reader),
                    &path,
                    bytes,
                    1,
                    usize::MAX,
                    TEXT_BYTES,
                ) {
                    Ok(read) => {
                        result["text"] = read["text"].clone();
                        result["truncated"] =
                            json!(read["truncated"] == true || read["more"] == true);
                    }
                    Err(e) if e.to_string().starts_with("Line ") => {
                        result["truncated"] = json!(true);
                        result["note"] = json!(
                            "This file contains a line that is too large for the bounded preview."
                        );
                    }
                    Err(e) => return Err(e),
                }
                Ok(result)
            })?
        } else {
            if remote.sftp.stat(&real)?["isDirectory"] != true {
                return Err(invalid("Choose a project folder"));
            }
            let collator = ordering::collator()?;
            let raw = remote.sftp.list_native(&real)?;
            let truncated = raw.len() > 500;
            let mut entries = raw
                .into_iter()
                .enumerate()
                .map(|(index, entry)| {
                    let name = entry["name"]
                        .as_str()
                        .ok_or_else(|| invalid("Invalid remote filename"))?
                        .to_owned();
                    if name.contains(['/', '\0']) || matches!(name.as_str(), "." | "..") {
                        return Err(invalid("Invalid remote filename"));
                    }
                    let kind = if entry["isDirectory"] == true {
                        "directory"
                    } else if entry["isFile"] == true {
                        "file"
                    } else {
                        "other"
                    };
                    let key = ordering::Key {
                        collator: &collator,
                        non_directory: kind != "directory",
                        name: name.clone(),
                        index,
                    };
                    Ok((
                        key,
                        json!({"name":name,"path":child(&path,&name),"kind":kind,"bytes":null}),
                    ))
                })
                .collect::<io::Result<Vec<_>>>()?;
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let entries = entries
                .into_iter()
                .take(500)
                .map(|(_, value)| value)
                .filter(|value| !matches!(value["name"].as_str(), Some(".git" | ".DS_Store")))
                .collect::<Vec<_>>();
            json!({"projectId":id,"projectName":name,"directory":path,"entries":entries,"truncated":truncated})
        };
        authorize()?;
        if remote.resolve(&path, false)? != real {
            return Err(invalid(
                "The project file changed location. Reload it before continuing.",
            ));
        }
        Ok(result)
    }
}
