//! ConPTY with verbatim cmd.exe arguments and job custody before the first instruction.
use crate::{
    process::{Launch, secret_environment},
    shell,
    tree::Tree,
};
use portable_pty::{Child, ChildKiller, ExitStatus, MasterPty, PtySize};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Write},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr,
    sync::{Arc, Mutex},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Console::*, Pipes::CreatePipe, Threading::*},
};
fn pipe() -> io::Result<(File, File)> {
    unsafe {
        let mut read = ptr::null_mut();
        let mut write = ptr::null_mut();
        if CreatePipe(&mut read, &mut write, ptr::null(), 0) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((File::from_raw_handle(read), File::from_raw_handle(write)))
    }
}
struct Console {
    hpc: HPCON,
}
// These kernel handles have no Rust memory aliasing and support cross-thread use.
unsafe impl Send for Console {}
unsafe impl Sync for Console {}
impl Drop for Console {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self.hpc);
        }
    }
}
struct Master {
    console: Arc<Console>,
    read: File,
    write: Mutex<Option<File>>,
    size: Mutex<PtySize>,
}
impl MasterPty for Master {
    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        let hr = unsafe {
            ResizePseudoConsole(
                self.console.hpc,
                COORD {
                    X: size.cols as i16,
                    Y: size.rows as i16,
                },
            )
        };
        if hr < 0 {
            return Err(anyhow::anyhow!("ConPTY resize failed: {hr}"));
        }
        *self.size.lock().unwrap() = size;
        Ok(())
    }
    fn get_size(&self) -> anyhow::Result<PtySize> {
        Ok(*self.size.lock().unwrap())
    }
    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn Read + Send>> {
        Ok(Box::new(self.read.try_clone()?))
    }
    fn take_writer(&self) -> anyhow::Result<Box<dyn Write + Send>> {
        Ok(Box::new(self.write.lock().unwrap().take().ok_or_else(
            || io::Error::other("PTY writer already taken"),
        )?))
    }
}
#[derive(Debug, Clone)]
struct NativeChild {
    handle: Arc<OwnedHandle>,
    pid: u32,
}
impl ChildKiller for NativeChild {
    fn kill(&mut self) -> io::Result<()> {
        if unsafe { TerminateProcess(self.handle.as_raw_handle(), 1) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(self.clone())
    }
}
impl Child for NativeChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.handle.as_raw_handle(), &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(Some(ExitStatus::with_exit_code(code)))
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    fn wait(&mut self) -> io::Result<ExitStatus> {
        unsafe {
            WaitForSingleObject(self.handle.as_raw_handle(), INFINITE);
        }
        self.try_wait()?
            .ok_or_else(|| io::Error::other("PTY wait did not settle"))
    }
    fn process_id(&self) -> Option<u32> {
        Some(self.pid)
    }
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        Some(self.handle.as_raw_handle())
    }
}
struct Attributes {
    storage: Vec<usize>,
}
impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.pointer());
        }
    }
}
fn quoted(value: &OsStr) -> Vec<u16> {
    let units = value.encode_wide().collect::<Vec<_>>();
    if !units.is_empty() && !units.iter().any(|c| [9, 10, 11, 32, 34].contains(c)) {
        return units;
    }
    let mut out = vec![34];
    let mut backslashes = 0;
    for unit in units {
        if unit == 92 {
            backslashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            92,
            if unit == 34 {
                backslashes * 2 + 1
            } else {
                backslashes
            },
        ));
        backslashes = 0;
        out.push(unit);
    }
    out.extend(std::iter::repeat_n(92, backslashes * 2));
    out.push(34);
    out
}
pub struct Spawn {
    // Kill the child and close its pipes before closing an unpublished ConPTY.
    pub tree: Tree,
    pub child: Box<dyn Child + Send + Sync>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
}
pub fn spawn(
    launch: &Launch<'_>,
    extra: Option<&BTreeMap<String, String>>,
    deterministic: bool,
) -> io::Result<Spawn> {
    let (input_read, input_write) = pipe()?;
    let (output_read, output_write) = pipe()?;
    let mut hpc = 0;
    let hr = unsafe {
        CreatePseudoConsole(
            COORD { X: 80, Y: 24 },
            input_read.as_raw_handle(),
            output_write.as_raw_handle(),
            0,
            &mut hpc,
        )
    };
    if hr < 0 {
        return Err(io::Error::other(format!("ConPTY creation failed: {hr}")));
    }
    let console = Arc::new(Console { hpc });
    drop(input_read);
    drop(output_write);
    let mut bytes = 0;
    unsafe {
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes);
    }
    let mut attributes = Attributes {
        storage: vec![0; bytes.div_ceil(std::mem::size_of::<usize>())],
    };
    if unsafe { InitializeProcThreadAttributeList(attributes.pointer(), 1, 0, &mut bytes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.pointer(),
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            hpc as *const _,
            std::mem::size_of::<HPCON>(),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let program = OsStr::new(launch.program);
    let mut cmdline = quoted(program);
    for (i, arg) in launch.args.iter().enumerate() {
        cmdline.push(32);
        if launch.raw_cmd && i == launch.args.len() - 1 {
            cmdline.extend(OsStr::new(arg).encode_wide());
        } else {
            cmdline.extend(quoted(OsStr::new(arg)));
        }
    }
    if cmdline.contains(&0) {
        return Err(io::Error::other("NUL in command line"));
    }
    cmdline.push(0);
    let mut executable = program.encode_wide().collect::<Vec<_>>();
    executable.push(0);
    let mut cwd = launch.cwd.as_os_str().encode_wide().collect::<Vec<_>>();
    cwd.push(0);
    let mut env = BTreeMap::<String, (OsString, OsString)>::new();
    for (key, value) in std::env::vars_os() {
        if if deterministic {
            !secret_environment(&key.to_string_lossy())
        } else {
            !key.to_string_lossy()
                .eq_ignore_ascii_case("ELECTRON_RUN_AS_NODE")
        } {
            env.insert(key.to_string_lossy().to_uppercase(), (key, value));
        }
    }
    if let Some(extra) = extra {
        for (key, value) in extra {
            env.insert(key.to_uppercase(), (key.into(), value.into()));
        }
    }
    if deterministic {
        for (key, value) in shell::environment().as_object().unwrap() {
            env.insert(
                key.to_uppercase(),
                (key.into(), value.as_str().unwrap().into()),
            );
        }
    }
    let mut environment = vec![];
    for (key, value) in env.into_values() {
        environment.extend(key.encode_wide());
        environment.push(61);
        environment.extend(value.encode_wide());
        environment.push(0);
    }
    environment.push(0);
    let mut info: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    info.StartupInfo.cb = std::mem::size_of_val(&info) as u32;
    info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    info.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
    info.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
    info.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
    info.lpAttributeList = attributes.pointer();
    let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe {
        CreateProcessW(
            executable.as_ptr(),
            cmdline.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &info.StartupInfo,
            &mut process,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut child = NativeChild {
        handle: Arc::new(unsafe { OwnedHandle::from_raw_handle(process.hProcess) }),
        pid: process.dwProcessId,
    };
    let thread = unsafe { OwnedHandle::from_raw_handle(process.hThread) };
    let tree = match Tree::attach(process.dwProcessId, false) {
        Ok(t) => t,
        Err(e) => {
            let _ = child.kill();
            return Err(e);
        }
    };
    let master = Box::new(Master {
        console,
        read: output_read,
        write: Mutex::new(Some(input_write)),
        size: Mutex::new(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }),
    });
    let reader = master.try_clone_reader().map_err(io::Error::other)?;
    let writer = master.take_writer().map_err(io::Error::other)?;
    if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
        let _ = child.kill();
        return Err(io::Error::last_os_error());
    }
    drop(thread);
    Ok(Spawn {
        child: Box::new(child),
        master,
        reader,
        writer,
        tree,
    })
}
