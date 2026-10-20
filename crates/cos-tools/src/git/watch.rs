//! Events invalidate a snapshot; their names never stand in for Git status.
use super::*;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct GitWatchSet {
    active: Option<Active>,
}
struct Active {
    project: String,
    context: Context,
    watcher: RecommendedWatcher,
    metadata: BTreeSet<PathBuf>,
    receiver: mpsc::Receiver<notify::Result<notify::Event>>,
    overflow: Arc<AtomicBool>,
    changed: Option<Instant>,
}
fn ordinary_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return false;
            }
        }
        m.is_dir() && !m.file_type().is_symlink()
    })
}
fn collect(path: &Path, out: &mut BTreeSet<PathBuf>) {
    if out.len() >= 96 || !ordinary_directory(path) {
        return;
    }
    out.insert(path.into());
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.take(4096).flatten() {
            if out.len() >= 96 {
                return;
            }
            if entry
                .file_type()
                .is_ok_and(|t| t.is_dir() && !t.is_symlink())
            {
                collect(&entry.path(), out);
            }
        }
    }
}
impl Active {
    fn refresh_metadata(&mut self) {
        let git = self.context.custody[2].0.clone();
        let common = self.context.custody[3].0.clone();
        let anchors = BTreeSet::from([git, common.clone()]);
        let mut candidates = anchors.clone();
        collect(&common.join("refs"), &mut candidates);
        collect(&common.join("logs/refs"), &mut candidates);
        // Refs/log subdirectories are disposable Git data, not repository
        // identity. A path may have been deleted and recreated within one
        // debounce interval, so rearm all optional watches even if it exists.
        // Keep the validated repository anchors continuously registered.
        self.metadata.retain(|path| {
            if anchors.contains(path) {
                true
            } else {
                let _ = self.watcher.unwatch(path);
                false
            }
        });
        for candidate in candidates {
            if !self.metadata.contains(&candidate)
                && ordinary_directory(&candidate)
                && self
                    .watcher
                    .watch(&candidate, RecursiveMode::NonRecursive)
                    .is_ok()
            {
                self.metadata.insert(candidate);
            }
        }
    }
}
impl GitWatchSet {
    pub fn sync(&mut self, roots: &[Root], path: &str, project: Option<&str>) -> io::Result<()> {
        if let Some(active) = &self.active
            && project == Some(active.project.as_str())
            && path == active.context.virtual_path
        {
            if let Err(e) = active.context.validate(roots) {
                self.close();
                return Err(e);
            }
            return Ok(());
        }
        // Dropping the receiver and native watcher discards every prior generation.
        self.close();
        let Some(project) = project else {
            return Ok(());
        };
        let Some(context) = context(roots, path)? else {
            return Ok(());
        };
        let (tx, receiver) = mpsc::sync_channel(64);
        let overflow = Arc::new(AtomicBool::new(false));
        let flag = overflow.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| {
                // Reads performed by refresh/preview must not trigger another
                // refresh or fill the bounded invalidation queue.
                if event.as_ref().is_ok_and(|e| e.kind.is_access()) {
                    return;
                }
                if tx.try_send(event).is_err() {
                    flag.store(true, Ordering::Release);
                }
            },
            Config::default().with_follow_symlinks(false),
        )
        .map_err(io::Error::other)?;
        watcher
            .watch(&context.real, RecursiveMode::Recursive)
            .map_err(io::Error::other)?;
        let mut active = Active {
            project: project.into(),
            context,
            watcher,
            metadata: BTreeSet::new(),
            receiver,
            overflow,
            changed: None,
        };
        active.refresh_metadata();
        active.context.validate(roots)?;
        self.active = Some(active);
        Ok(())
    }
    pub fn poll(&mut self, roots: &[Root]) -> io::Result<Option<Value>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        if let Err(e) = active.context.validate(roots) {
            self.close();
            return Err(e);
        }
        if active.overflow.swap(false, Ordering::AcqRel) {
            active.changed = Some(Instant::now());
        }
        for event in active.receiver.try_iter().take(64) {
            event.map_err(io::Error::other)?;
            active.changed = Some(Instant::now());
        }
        if active
            .changed
            .is_some_and(|at| at.elapsed() >= Duration::from_millis(180))
        {
            active.changed = None;
            active.refresh_metadata();
            return Ok(Some(json!({"projectId":active.project})));
        }
        Ok(None)
    }
    pub fn close(&mut self) {
        self.active = None;
    }
}
