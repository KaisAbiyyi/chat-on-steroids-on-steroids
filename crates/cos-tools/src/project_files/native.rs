use super::*;
fn number(n: f64) -> Value {
    if n.fract() == 0. && n >= 0. && n < u64::MAX as f64 {
        json!(n as u64)
    } else {
        json!(n)
    }
}
pub(super) fn stamp(file: &File) -> io::Result<Value> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::*;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        let mut basic: FILE_BASIC_INFO = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
            || unsafe {
                GetFileInformationByHandleEx(
                    file.as_raw_handle(),
                    FileBasicInfo,
                    (&mut basic as *mut FILE_BASIC_INFO).cast(),
                    std::mem::size_of_val(&basic) as u32,
                )
            } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let ms = |ticks: i64| {
            let t = ticks - 116_444_736_000_000_000;
            (t / 10_000_000) as f64 * 1000. + (t % 10_000_000) as f64 / 10_000.
        };
        Ok(json!([
            number(info.dwVolumeSerialNumber as f64),
            number(((u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)) as f64),
            file.metadata()?.len(),
            number(ms(basic.LastWriteTime)),
            number(ms(basic.ChangeTime))
        ]))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        Ok(json!([
            number(m.dev() as f64),
            number(m.ino() as f64),
            m.len(),
            number(m.mtime() as f64 * 1000. + m.mtime_nsec() as f64 / 1_000_000.),
            number(m.ctime() as f64 * 1000. + m.ctime_nsec() as f64 / 1_000_000.)
        ]))
    }
}
pub(super) fn rename_handle(parent: &Dir, name: &std::ffi::OsStr) -> io::Result<File> {
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .access_mode(0x10080)
            .custom_flags(0x02200000);
        Ok(parent.open_with(name, &options)?.into_std())
    }
    #[cfg(unix)]
    {
        let metadata = parent.symlink_metadata(name)?;
        if metadata.is_dir() {
            Ok(parent.open_dir(name)?.into_std_file())
        } else {
            Ok(parent.open(name)?.into_std())
        }
    }
}
pub(super) fn rename(
    source: &File,
    old_dir: &Dir,
    old: &std::ffi::OsStr,
    new_dir: &Dir,
    new: &std::ffi::OsStr,
    replace: bool,
) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
        use windows_sys::{
            Wdk::Storage::FileSystem::{
                FILE_RENAME_INFORMATION, FileRenameInformation, NtSetInformationFile,
            },
            Win32::{
                Foundation::RtlNtStatusToDosError,
                System::{
                    IO::IO_STATUS_BLOCK,
                    Threading::{INFINITE, WaitForSingleObject},
                },
            },
        };
        let _ = (old_dir, old);
        let name = new.encode_wide().collect::<Vec<_>>();
        let offset = std::mem::offset_of!(FILE_RENAME_INFORMATION, FileName);
        let size = std::mem::size_of::<FILE_RENAME_INFORMATION>() + name.len() * 2;
        let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
        let directory = new_dir.try_clone()?.into_std_file();
        // Aligned variable-length storage; the source and parent stay alive for the call.
        // kernel32's wrapper rewrites relative names to absolute names, which
        // conflicts with RootDirectory. The NT operation keeps the held anchor.
        unsafe {
            (*info).Anonymous.ReplaceIfExists = replace;
            (*info).RootDirectory = directory.as_raw_handle();
            (*info).FileNameLength = (name.len() * 2) as u32;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
                name.len(),
            );
            let mut io_status: Box<IO_STATUS_BLOCK> = Box::new(std::mem::zeroed());
            let mut status = NtSetInformationFile(
                source.as_raw_handle(),
                &mut *io_status,
                info.cast(),
                size as u32,
                FileRenameInformation,
            );
            if status == 0x103 {
                if WaitForSingleObject(source.as_raw_handle(), INFINITE) != 0 {
                    let error = io::Error::last_os_error();
                    // An outstanding native operation must never retain freed buffers.
                    std::mem::forget(buffer);
                    std::mem::forget(io_status);
                    return Err(error);
                }
                status = io_status.Anonymous.Status;
            }
            if status < 0 {
                return Err(io::Error::from_raw_os_error(
                    RtlNtStatusToDosError(status) as i32
                ));
            }
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
        let _ = source;
        let old =
            std::ffi::CString::new(old.as_bytes()).map_err(|_| invalid("Invalid filename"))?;
        let new =
            std::ffi::CString::new(new.as_bytes()).map_err(|_| invalid("Invalid filename"))?;
        let old_dir = old_dir.try_clone()?.into_std_file();
        let new_dir = new_dir.try_clone()?.into_std_file();
        if unsafe {
            libc::renameat2(
                old_dir.as_raw_fd(),
                old.as_ptr(),
                new_dir.as_raw_fd(),
                new.as_ptr(),
                if replace { 0 } else { libc::RENAME_NOREPLACE },
            )
        } != 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = (source, old_dir, old, new_dir, new, replace);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Native project file mutations on this platform are on hold",
        ))
    }
}
