//! Process tree lifetime. Pipe children are suspended until assigned to a Windows job.
use std::io;
#[cfg(any(unix, test))]
#[derive(Default)]
struct GroupCustody {
    retired: bool,
}
#[cfg(any(unix, test))]
impl GroupCustody {
    fn terminate(&mut self, owned: impl FnOnce() -> bool, signal: impl FnOnce()) {
        if !self.retired {
            self.retired = true;
            if owned() {
                signal();
            }
        }
    }
}
#[cfg(windows)]
pub struct Tree {
    job: windows_sys::Win32::Foundation::HANDLE,
}
#[cfg(windows)]
unsafe impl Send for Tree {}
#[cfg(windows)]
unsafe impl Sync for Tree {}
#[cfg(windows)]
impl Tree {
    pub fn ready_to_reap(&self) -> io::Result<bool> {
        Ok(true)
    }
    pub fn memory_limit(&self, bytes: usize) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::*;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        limits.ProcessMemoryLimit = bytes;
        let ok = unsafe {
            SetInformationJobObject(
                self.job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    pub fn attach(pid: u32, resume: bool) -> io::Result<Self> {
        use windows_sys::Win32::{
            Foundation::*,
            System::{Diagnostics::ToolHelp::*, JobObjects::*, Threading::*},
        };
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                CloseHandle(job);
                return Err(io::Error::last_os_error());
            }
            let process = OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            );
            if process.is_null() {
                CloseHandle(job);
                return Err(io::Error::last_os_error());
            }
            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return Err(io::Error::last_os_error());
            }
            let guard = Self { job };
            if resume {
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                if snapshot == INVALID_HANDLE_VALUE {
                    return Err(io::Error::last_os_error());
                }
                let mut entry: THREADENTRY32 = std::mem::zeroed();
                entry.dwSize = std::mem::size_of_val(&entry) as u32;
                let mut found = false;
                let mut has = Thread32First(snapshot, &mut entry);
                while has != 0 {
                    if entry.th32OwnerProcessID == pid {
                        let t = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                        if !t.is_null() {
                            found |= ResumeThread(t) != u32::MAX;
                            CloseHandle(t);
                        }
                    }
                    has = Thread32Next(snapshot, &mut entry);
                }
                CloseHandle(snapshot);
                if !found {
                    return Err(io::Error::other("Unable to resume guarded child"));
                }
            }
            Ok(guard)
        }
    }
    pub fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }
}
#[cfg(windows)]
impl Drop for Tree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}
#[cfg(unix)]
pub struct Tree {
    pid: i32,
    custody: std::sync::Mutex<GroupCustody>,
}
#[cfg(unix)]
impl Tree {
    pub fn memory_limit(&self, _bytes: usize) -> io::Result<()> {
        Ok(())
    }
    pub fn attach(pid: u32, _resume: bool) -> io::Result<Self> {
        let pid = i32::try_from(pid).map_err(io::Error::other)?;
        if pid <= 0 {
            return Err(io::Error::other("Invalid process group leader"));
        }
        let tree = Self {
            pid,
            custody: std::sync::Mutex::new(GroupCustody::default()),
        };
        tree.peek()?;
        Ok(tree)
    }
    // WNOWAIT leaves the dead leader waitable, reserving its PID until group cleanup.
    fn peek(&self) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        loop {
            if unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
                )
            } == 0
            {
                return Ok(unsafe { info.si_pid() } != 0);
            }
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }
    fn signal(&self, signal: i32) {
        unsafe {
            libc::kill(-self.pid, signal);
        }
    }
    pub fn ready_to_reap(&self) -> io::Result<bool> {
        let mut custody = self
            .custody
            .lock()
            .map_err(|_| io::Error::other("Process custody lock poisoned"))?;
        match self.peek() {
            Ok(true) => {
                custody.terminate(|| true, || self.signal(libc::SIGKILL));
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => {
                custody.retired = true;
                Err(e)
            }
        }
    }
    pub fn terminate(&self) {
        if let Ok(mut custody) = self.custody.lock() {
            custody.terminate(|| self.peek().is_ok(), || self.signal(libc::SIGKILL));
        }
    }
    pub fn interrupt(&self) {
        if let Ok(mut custody) = self.custody.lock() {
            if !custody.retired {
                if self.peek().is_ok() {
                    self.signal(libc::SIGINT);
                } else {
                    custody.retired = true;
                }
            }
        }
    }
}
#[cfg(unix)]
impl Drop for Tree {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Model only the unavailable POSIX ownership probe; exercise the actual custody state.
    #[test]
    fn group_termination_is_one_shot_after_leader_reaping() {
        let mut custody = GroupCustody::default();
        let sent = std::cell::Cell::new(0);
        custody.terminate(|| true, || sent.set(sent.get() + 1));
        custody.terminate(|| false, || sent.set(sent.get() + 1));
        custody.terminate(|| true, || sent.set(sent.get() + 1));
        assert_eq!(
            sent.get(),
            1,
            "retained session signalled a retired process group"
        );
    }
    #[test]
    fn lost_child_custody_never_signals_a_reused_group() {
        let mut custody = GroupCustody::default();
        let sent = std::cell::Cell::new(0);
        custody.terminate(|| false, || sent.set(sent.get() + 1));
        assert_eq!(sent.get(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn exited_leader_is_not_reaped_before_descendant_pipe_cleanup() {
        use std::{
            io::Read,
            os::unix::process::CommandExt,
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let mut child = Command::new("sh")
            .args(["-c", "sleep 30 & printf public; exit 0"])
            .process_group(0)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let tree = Tree::attach(child.id(), false).unwrap();
        let mut output = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            output.read_to_end(&mut bytes).unwrap();
            tx.send(bytes).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while !tree.ready_to_reap().unwrap() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(child.wait().unwrap().success());
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), b"public");
        tree.terminate(); // retained completed sessions must now be inert.
    }
}
