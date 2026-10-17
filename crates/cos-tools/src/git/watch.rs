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
    _watcher: RecommendedWatcher,
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
        let Some(mut context) = context(roots, path)? else {
            return Ok(());
        };
        let (tx, receiver) = mpsc::sync_channel(64);
        let overflow = Arc::new(AtomicBool::new(false));
        let flag = overflow.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event| {
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
        let git = context.custody[2].0.clone();
        let common = context.custody[3].0.clone();
        let mut candidates = BTreeSet::from([git, common.clone()]);
        collect(&common.join("refs"), &mut candidates);
        collect(&common.join("logs/refs"), &mut candidates);
        for candidate in candidates {
            if !ordinary_directory(&candidate) {
                continue;
            }
            let held = directory_handle(&candidate)?;
            if watcher
                .watch(&candidate, RecursiveMode::NonRecursive)
                .is_ok()
            {
                context.custody.push((candidate, held));
            }
        }
        context.validate(roots)?;
        self.active = Some(Active {
            project: project.into(),
            context,
            _watcher: watcher,
            receiver,
            overflow,
            changed: None,
        });
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
            return Ok(Some(json!({"projectId":active.project})));
        }
        Ok(None)
    }
    pub fn close(&mut self) {
        self.active = None;
    }
}
