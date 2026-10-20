use cos_core::config::Root;
use cos_tools::{
    files::Files,
    search::{
        Options,
        engine::{self, Backend, Entry, Kind, Metadata},
    },
};
use serde_json::json;
use std::{
    fs, io,
    path::Path,
    time::{Duration, Instant},
};

fn roots(path: &Path) -> Vec<Root> {
    vec![Root {
        name: "p".into(),
        path: fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }]
}

fn denied(kind: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("{kind} authority denied"),
    )
}

#[test]
fn delete_requires_live_delete_file_authority_without_mutating_bytes() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("public.txt"), b"original\r\n").unwrap();
    let files = Files::default();
    let roots = roots(temp.path());
    let patch = "*** Begin Patch\n*** Delete File: public.txt\n*** End Patch";
    let error = files
        .apply_checked(&roots, "/p", patch, None, &|kind| {
            if kind == "deleteFile" {
                Err(denied(kind))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        fs::read(temp.path().join("public.txt")).unwrap(),
        b"original\r\n"
    );
    let receipt = files
        .apply_checked(&roots, "/p", patch, None, &|_| Ok(()))
        .unwrap();
    assert_eq!(receipt["transaction"], "committed");
    assert_eq!(receipt["changes"][0]["kind"], "delete");
    assert!(!temp.path().join("public.txt").exists());
}

#[test]
fn content_preserving_move_still_requires_move_authority() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("original.txt"), b"same\n").unwrap();
    let files = Files::default();
    let roots = roots(temp.path());
    let patch = "*** Begin Patch\n*** Update File: original.txt\n*** Move to: moved.txt\n@@\n same\n*** End Patch";
    let error = files
        .apply_checked(&roots, "/p", patch, None, &|kind| {
            if kind == "move" {
                Err(denied(kind))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        fs::read(temp.path().join("original.txt")).unwrap(),
        b"same\n"
    );
    assert!(!temp.path().join("moved.txt").exists());
    let receipt = files
        .apply_checked(&roots, "/p", patch, None, &|_| Ok(()))
        .unwrap();
    assert_eq!(receipt["transaction"], "committed");
    assert_eq!(fs::read(temp.path().join("moved.txt")).unwrap(), b"same\n");
    assert!(!temp.path().join("original.txt").exists());
}

#[test]
fn denied_later_patch_operation_preserves_all_earlier_hunks() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("first.txt"), b"before\n").unwrap();
    fs::write(temp.path().join("second.txt"), b"retained\n").unwrap();
    let patch = "*** Begin Patch\n*** Update File: first.txt\n@@\n-before\n+after\n*** Delete File: second.txt\n*** End Patch";
    let error = Files::default()
        .apply_checked(&roots(temp.path()), "/p", patch, None, &|kind| {
            if kind == "deleteFile" {
                Err(denied(kind))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        fs::read(temp.path().join("first.txt")).unwrap(),
        b"before\n"
    );
    assert_eq!(
        fs::read(temp.path().join("second.txt")).unwrap(),
        b"retained\n"
    );
}

enum Failure {
    FolderPermission,
    FolderMissing,
    FilePermission,
    MetadataPermission,
}

struct Probe {
    failure: Failure,
    metadata_reads: usize,
}

impl Backend for Probe {
    fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
        self.metadata_reads += 1;
        if matches!(self.failure, Failure::MetadataPermission) && self.metadata_reads > 1 {
            return Err(denied("metadata"));
        }
        match path {
            "/p" if matches!(
                self.failure,
                Failure::FolderPermission | Failure::FolderMissing
            ) =>
            {
                Ok(Metadata {
                    kind: Kind::Directory,
                    bytes: 0,
                })
            }
            "/p" | "/p/visible.txt"
                if matches!(
                    self.failure,
                    Failure::FilePermission | Failure::MetadataPermission
                ) =>
            {
                Ok(Metadata {
                    kind: Kind::File,
                    bytes: 7,
                })
            }
            _ => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }
    fn open_folder(
        &mut self,
        path: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
        match path {
            "/p" => Ok(Box::new(
                vec![
                    Ok(Entry {
                        name: "visible.txt".into(),
                        kind: Kind::File,
                    }),
                    Ok(Entry {
                        name: "restricted".into(),
                        kind: Kind::Directory,
                    }),
                ]
                .into_iter(),
            )),
            "/p/restricted" => Err(match self.failure {
                Failure::FolderPermission => denied("folder"),
                _ => io::Error::from(io::ErrorKind::NotFound),
            }),
            _ => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }
    fn read_file(
        &mut self,
        _: &str,
        _: usize,
        _: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        match self.failure {
            Failure::FilePermission => Err(denied("file")),
            _ => Ok(b"visible".to_vec()),
        }
    }
}

#[test]
fn missing_folder_is_skipped_but_denied_folder_cannot_publish_partial_results() {
    let mut missing = Probe {
        failure: Failure::FolderMissing,
        metadata_reads: 0,
    };
    let outcome = engine::search(
        &mut missing,
        "/p",
        &Options::new("visible"),
        Instant::now() + Duration::from_secs(5),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(json!(outcome.hits)[0]["path"], "/p/visible.txt");
    assert!(!outcome.truncated);
    let mut denied_folder = Probe {
        failure: Failure::FolderPermission,
        metadata_reads: 0,
    };
    let error = engine::search(
        &mut denied_folder,
        "/p",
        &Options::new("visible"),
        Instant::now() + Duration::from_secs(5),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "folder authority denied");
}

#[test]
fn denied_content_and_metadata_must_not_be_reported_as_skipped() {
    for failure in [Failure::FilePermission, Failure::MetadataPermission] {
        let mut backend = Probe {
            failure,
            metadata_reads: 0,
        };
        let error = engine::search(
            &mut backend,
            "/p",
            &Options {
                content: true,
                ..Options::new("visible")
            },
            Instant::now() + Duration::from_secs(5),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
