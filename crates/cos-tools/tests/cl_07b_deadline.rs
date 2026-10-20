use cos_tools::search::{
    Options,
    engine::{self, Backend, Entry, Kind, Metadata, StopReason},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct Slow {
    read_calls: Arc<AtomicUsize>,
}
impl Backend for Slow {
    fn fork_reader(&self) -> Option<Box<dyn Backend>> {
        Some(Box::new(Self { read_calls: self.read_calls.clone() }))
    }
    fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
        Ok(Metadata {
            kind: if path == "/p" {
                Kind::Directory
            } else {
                Kind::File
            },
            bytes: if path == "/p" { 0 } else { 6 },
        })
    }
    fn open_folder(
        &mut self,
        _: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
        Ok(Box::new(
            vec![Ok(Entry {
                name: "needle.txt".into(),
                kind: Kind::File,
            })]
            .into_iter(),
        ))
    }
    fn read_file(
        &mut self,
        _: &str,
        _: usize,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        authorize()?;
        self.read_calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(150));
        authorize()?;
        Ok(b"needle".to_vec())
    }
}
#[test]
fn late_content_worker_does_not_publish_after_shared_deadline() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut backend = Slow {
        read_calls: calls.clone(),
    };
    let output = engine::search(
        &mut backend,
        "/p",
        &Options {
            content: true,
            ..Options::new("needle")
        },
        Instant::now() + Duration::from_millis(80),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(output.stopped_because, Some(StopReason::Time));
    assert!(output.truncated);
    assert!(
        output.hits.is_empty(),
        "Late hits must not claim timely publication"
    );
}
