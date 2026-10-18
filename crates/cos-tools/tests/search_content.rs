use base64::{Engine, engine::general_purpose::STANDARD};
use cos_core::config::Root;
use cos_tools::search::{Options, engine, native::Native};
use serde_json::Value;
use std::{
    fs, io,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
#[test]
fn native_content_search_retains_source_decoder_sniff_lines_and_explicit_file_limits() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/search-content.json"
    ))
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let roots = [Root {
        name: "p".into(),
        path: fs::canonicalize(directory.path())
            .unwrap()
            .to_string_lossy()
            .into(),
        remote: None,
    }];
    let mut qualified = 0;
    let mut lossless_gap = 0;
    for row in corpus["content"].as_array().unwrap() {
        let bytes = if let Some(count) = row["zeroBytes"].as_u64() {
            vec![0; count as usize]
        } else {
            STANDARD.decode(row["bytes"].as_str().unwrap()).unwrap()
        };
        fs::write(directory.path().join("a.txt"), bytes).unwrap();
        let result = engine::search(
            &mut Native::new(&roots),
            "/p/a.txt",
            &Options {
                content: true,
                limit: row["maxResults"].as_u64().unwrap_or(50) as usize,
                ..Options::new(row["query"].as_str().unwrap())
            },
            Instant::now() + Duration::from_secs(10),
            &|| Ok(()),
        );
        if row["needsLosslessString"] == true {
            // Keep the actual source's unpaired code unit in the corpus; do not substitute U+FFFD.
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
            lossless_gap += 1;
            continue;
        }
        let mut projected = serde_json::to_value(result.unwrap()).unwrap();
        projected.as_object_mut().unwrap().remove("elapsedMs");
        for hit in projected["hits"].as_array_mut().unwrap() {
            if let Some(text) = hit.get("text").and_then(Value::as_str) {
                let units: Vec<_> = text.encode_utf16().collect();
                hit.as_object_mut().unwrap().remove("text");
                hit["textUnits"] = serde_json::to_value(units).unwrap();
            }
        }
        assert_eq!(projected, row["expected"], "{row}");
        qualified += 1;
    }
    assert_eq!(
        (qualified, lossless_gap),
        (11, 1),
        "Lossless JSON strings remain an explicit whole-workspace gap"
    );
}

#[test]
fn native_content_jobs_overlap_twelve_reads_without_moving_caller_authority() {
    use engine::{Backend, Entry, Metadata};
    struct Gate {
        state: Mutex<(usize, bool)>,
        ready: Condvar,
    }
    struct Gated {
        reader: Box<dyn Backend>,
        gate: Arc<Gate>,
    }
    impl Backend for Gated {
        fn fork_reader(&self) -> Option<Box<dyn Backend>> {
            Some(Box::new(Self {
                reader: self.reader.fork_reader().unwrap(),
                gate: self.gate.clone(),
            }))
        }
        fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
            self.reader.metadata(path)
        }
        fn open_folder(
            &mut self,
            path: &str,
        ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
            self.reader.open_folder(path)
        }
        fn read_file(
            &mut self,
            path: &str,
            max: usize,
            authorize: &dyn Fn() -> io::Result<()>,
        ) -> io::Result<Vec<u8>> {
            {
                let mut state = self.gate.state.lock().unwrap();
                state.0 += 1;
                if state.0 == 12 {
                    state.1 = true;
                    self.gate.ready.notify_all();
                }
                let (_state, timeout) = self
                    .gate
                    .ready
                    .wait_timeout_while(state, Duration::from_secs(3), |state| !state.1)
                    .unwrap();
                if timeout.timed_out() {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "All twelve native content jobs must enter before any read completes",
                    ));
                }
            }
            self.reader.read_file(path, max, authorize)
        }
    }
    let directory = tempfile::tempdir().unwrap();
    for index in 0..12 {
        fs::write(
            directory.path().join(format!("public-{index}.txt")),
            "public needle\n",
        )
        .unwrap();
    }
    let roots = [Root {
        name: "p".into(),
        path: fs::canonicalize(directory.path())
            .unwrap()
            .to_string_lossy()
            .into(),
        remote: None,
    }];
    for revoked in [false, true] {
        let gate = Arc::new(Gate {
            state: Mutex::new((0, false)),
            ready: Condvar::new(),
        });
        let mut backend = Gated {
            reader: Native::new(&roots).fork_reader().unwrap(),
            gate: gate.clone(),
        };
        let caller = std::thread::current().id();
        let checks = std::cell::Cell::new(0usize);
        let output = engine::search(
            &mut backend,
            "/p",
            &Options {
                content: true,
                ..Options::new("needle")
            },
            Instant::now() + Duration::from_secs(10),
            &|| {
                assert_eq!(
                    std::thread::current().id(),
                    caller,
                    "Request authority must be checked on its owner thread"
                );
                checks.set(checks.get() + 1);
                if revoked && gate.state.lock().unwrap().1 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Native content authority revoked with twelve jobs in flight",
                    ));
                }
                Ok(())
            },
        );
        if revoked {
            assert_eq!(output.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        } else {
            let output = output.unwrap();
            assert_eq!(output.hits.len(), 12);
            assert_eq!(output.files_scanned, 12);
            assert!(!output.truncated);
        }
        assert_eq!(gate.state.lock().unwrap().0, 12);
        assert!(checks.get() > 12);
    }
}
