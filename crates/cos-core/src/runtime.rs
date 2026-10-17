//! Cooperating owners stop accepting work before bounded shutdown phases run.
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> io::Result<()> {
        if self.is_cancelled() {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Runtime is shutting down",
            ))
        } else {
            Ok(())
        }
    }
}
pub struct ShutdownPhase {
    pub name: String,
    pub budget: Duration,
    run: Box<dyn FnOnce() -> io::Result<()> + Send>,
}
impl ShutdownPhase {
    pub fn new(
        name: &str,
        budget: Duration,
        run: impl FnOnce() -> io::Result<()> + Send + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            budget,
            run: Box::new(run),
        }
    }
}
#[derive(serde::Serialize)]
pub struct ShutdownResult {
    pub phase: String,
    pub status: &'static str,
}
/// A timed-out native call cannot be forcibly unwound. The report retains that fact;
/// the application host owns final process exit after later phases have been attempted.
pub fn shutdown(cancel: &Cancellation, phases: Vec<ShutdownPhase>) -> Vec<ShutdownResult> {
    cancel.cancel();
    phases
        .into_iter()
        .map(|phase| {
            let (send, receive) = mpsc::sync_channel(1);
            let started = thread::Builder::new()
                .name(format!("shutdown-{}", phase.name))
                .spawn(move || {
                    let status = match catch_unwind(AssertUnwindSafe(phase.run)) {
                        Ok(Ok(())) => "ok",
                        _ => "failed",
                    };
                    let _ = send.send(status);
                });
            let status = if started.is_err() {
                "failed"
            } else {
                match receive.recv_timeout(phase.budget) {
                    Ok(status) => status,
                    Err(mpsc::RecvTimeoutError::Timeout) => "timeout",
                    Err(_) => "failed",
                }
            };
            ShutdownResult {
                phase: phase.name,
                status,
            }
        })
        .collect()
}

#[cfg(windows)]
pub struct AwakeGuard(std::marker::PhantomData<std::rc::Rc<()>>);
#[cfg(windows)]
impl AwakeGuard {
    pub fn acquire() -> io::Result<Self> {
        use windows_sys::Win32::System::Power::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        if unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(std::marker::PhantomData))
        }
    }
}
#[cfg(windows)]
impl Drop for AwakeGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Power::{ES_CONTINUOUS, SetThreadExecutionState};
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}
