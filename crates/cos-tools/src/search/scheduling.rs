//! Bounded content workers; authorization and result publication remain on the calling thread.
use super::{
    Options,
    engine::{self, Backend, Entry, Hit, Metadata, Outcome, StopReason},
};
use cos_core::persistence::lock;
use regex::Regex;
use std::{
    io,
    sync::{Mutex, mpsc},
    time::Instant,
};

enum Event {
    Authorize(mpsc::SyncSender<io::Result<()>>),
    Completed(io::Result<(Vec<Hit>, usize)>),
    Next(mpsc::SyncSender<Option<String>>),
    Exited,
}
struct Shared<'a, 'b>(&'a Mutex<&'b mut dyn Backend>);
impl Backend for Shared<'_, '_> {
    fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
        lock(self.0)?.metadata(path)
    }
    fn open_folder(
        &mut self,
        _: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Content workers do not enumerate directories",
        ))
    }
    fn read_file(
        &mut self,
        path: &str,
        max: usize,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        lock(self.0)?.read_file(path, max, authorize)
    }
}
pub(super) fn scan(
    backend: &mut dyn Backend,
    candidates: Vec<String>,
    options: &Options<'_>,
    expression: Option<&Regex>,
    deadline: Instant,
    authorize: &dyn Fn() -> io::Result<()>,
    outcome: &mut Outcome,
) -> io::Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    authorize()?;
    if Instant::now() > deadline {
        outcome.stop(StopReason::Time);
        return Ok(());
    }
    let count = candidates.len().min(12);
    // The source admits every initial worker before any awaited file completes.
    let readers: Vec<_> = (0..count).map(|_| backend.fork_reader()).collect();
    let shared = Mutex::new(backend);
    let (send, receive) = mpsc::sync_channel::<Event>(48);
    let mut failure = None;
    let mut next = count;
    let mut done = false;
    std::thread::scope(|scope| {
        for (initial, reader) in candidates[..count].iter().cloned().zip(readers) {
            let sender = send.clone();
            let shared = &shared;
            scope.spawn(move || {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut shared_reader = Shared(shared);
                    let mut independent = reader;
                    let backend: &mut dyn Backend = match independent.as_deref_mut() {
                        Some(reader) => reader,
                        None => &mut shared_reader,
                    };
                    let authorize = || {
                        let (reply, result) = mpsc::sync_channel(1);
                        sender.send(Event::Authorize(reply)).map_err(|_| {
                            io::Error::new(io::ErrorKind::Interrupted, "Search coordinator closed")
                        })?;
                        result.recv().map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::Interrupted,
                                "Search authorization closed",
                            )
                        })?
                    };
                    let mut job = Some(initial);
                    while let Some(path) = job {
                        let mut skipped = 0;
                        let result = engine::scan_file(
                            backend,
                            &path,
                            options,
                            expression,
                            &authorize,
                            &mut skipped,
                        )
                        .map(|hits| (hits, skipped));
                        if sender.send(Event::Completed(result)).is_err() {
                            break;
                        }
                        let (reply, answer) = mpsc::sync_channel(1);
                        if sender.send(Event::Next(reply)).is_err() {
                            break;
                        }
                        job = answer.recv().ok().flatten();
                    }
                }));
                if run.is_err() {
                    let _ = sender.send(Event::Completed(Err(io::Error::other(
                        "Search content worker panicked",
                    ))));
                }
                let _ = sender.send(Event::Exited);
            });
        }
        drop(send);
        let mut workers = count;
        while workers > 0 {
            let event = match receive.recv() {
                Ok(event) => event,
                Err(_) => {
                    failure
                        .get_or_insert_with(|| io::Error::other("Search content channel closed"));
                    break;
                }
            };
            match event {
                Event::Authorize(reply) => {
                    let verdict = if failure.is_some() {
                        Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "Search was cancelled",
                        ))
                    } else {
                        authorize()
                    };
                    if let Err(error) = &verdict {
                        if failure.is_none() {
                            failure = Some(io::Error::new(error.kind(), error.to_string()));
                        }
                        done = true;
                    }
                    let _ = reply.send(verdict);
                }
                Event::Completed(result) => match result {
                    Ok((hits, skipped)) if failure.is_none() => {
                        if let Err(error) = authorize() {
                            failure = Some(error);
                            done = true;
                            continue;
                        }
                        // Work already in flight may finish after the shared deadline.
                        // Keep previously published hits but never claim late results
                        // were delivered within the search time budget.
                        if Instant::now() > deadline {
                            outcome.stop(StopReason::Time);
                            done = true;
                            continue;
                        }
                        outcome.skipped += skipped;
                        for hit in hits {
                            if outcome.hits.len() >= options.limit {
                                outcome.stop(StopReason::Limit);
                                done = true;
                                break;
                            }
                            outcome.hits.push(hit);
                        }
                    }
                    Err(error) => {
                        if failure.is_none() {
                            failure = Some(error);
                        }
                        done = true;
                    }
                    _ => {}
                },
                Event::Next(reply) => {
                    let job = if done {
                        None
                    } else if Instant::now() > deadline {
                        outcome.stop(StopReason::Time);
                        done = true;
                        None
                    } else {
                        let item = candidates.get(next).cloned();
                        next += 1;
                        if item.is_some() && outcome.hits.len() >= options.limit {
                            outcome.stop(StopReason::Limit);
                            done = true;
                            None
                        } else {
                            item
                        }
                    };
                    let _ = reply.send(job);
                }
                Event::Exited => workers -= 1,
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    authorize()
}
