#![cfg(windows)]

use std::ffi::{OsStr, c_void};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList, PROCESS_INFORMATION,
    LPPROC_THREAD_ATTRIBUTE_LIST, STARTUPINFOEXW, UpdateProcThreadAttribute,
    WaitForSingleObject,
};

type Hpcon = HANDLE;

const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x0002_0016;
const S_OK: i32 = 0;
const STILL_ACTIVE: u32 = 259;
const WAIT_TIMEOUT: u32 = 258;

type CreatePseudoConsoleFn = unsafe extern "system" fn(
    Coord,
    HANDLE,
    HANDLE,
    u32,
    *mut Hpcon,
) -> i32;
type ResizePseudoConsoleFn = unsafe extern "system" fn(Hpcon, Coord) -> i32;
type ClosePseudoConsoleFn = unsafe extern "system" fn(Hpcon);

#[repr(C)]
#[derive(Clone, Copy)]
struct Coord {
    x: i16,
    y: i16,
}

pub(super) struct ConPtyProcess {
    hpc: Hpcon,
    process: HANDLE,
    process_thread: HANDLE,
    pty_in: HANDLE,
    pty_out: HANDLE,
    cmd_in: HANDLE,
    cmd_out: HANDLE,
    closed: AtomicBool,
}

// Win32 HANDLEs and HPCON values are opaque OS-owned resources. Moving the wrapper across threads
// is the intended usage model for the ConPTY reader/writer helper threads.
unsafe impl Send for ConPtyProcess {}
unsafe impl Sync for ConPtyProcess {}

impl ConPtyProcess {
    pub(super) fn spawn(command_line: &str, cols: u16, rows: u16) -> io::Result<Arc<Self>> {
        if !is_conpty_available() {
            return Err(io::Error::other(
                "ConPty is not available on this version of Windows",
            ));
        }

        let mut sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 0,
        };

        let mut pty_in: HANDLE = null_mut();
        let mut cmd_in: HANDLE = null_mut();
        if unsafe { CreatePipe(&mut pty_in, &mut cmd_in, &mut sa, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut cmd_out: HANDLE = null_mut();
        let mut pty_out: HANDLE = null_mut();
        if unsafe { CreatePipe(&mut cmd_out, &mut pty_out, &mut sa, 0) } == 0 {
            unsafe {
                CloseHandle(pty_in);
                CloseHandle(cmd_in);
            }
            return Err(io::Error::last_os_error());
        }

        let coord = Coord {
            x: cols as i16,
            y: rows as i16,
        };
        // ConPTY is built from two anonymous pipes plus a pseudo console handle: one direction
        // feeds bytes into the child, the other reads terminal output back out.
        let hpc = match create_pseudo_console(coord, pty_in, pty_out) {
            Ok(hpc) => hpc,
            Err(err) => {
                unsafe {
                    CloseHandle(pty_in);
                    CloseHandle(pty_out);
                    CloseHandle(cmd_in);
                    CloseHandle(cmd_out);
                }
                return Err(err);
            }
        };

        let created = create_process_attached_to_pty(hpc, command_line);
        let pi = match created {
            Ok(pi) => pi,
            Err(err) => {
                unsafe {
                    close_pseudo_console(hpc);
                    CloseHandle(pty_in);
                    CloseHandle(pty_out);
                    CloseHandle(cmd_in);
                    CloseHandle(cmd_out);
                }
                return Err(err);
            }
        };

        Ok(Arc::new(Self {
            hpc,
            process: pi.hProcess,
            process_thread: pi.hThread,
            pty_in,
            pty_out,
            cmd_in,
            cmd_out,
            closed: AtomicBool::new(false),
        }))
    }

    pub(super) fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        let resize = resize_pseudo_console_ptr()?;
        let status = unsafe {
            resize(
                self.hpc,
                Coord {
                    x: cols as i16,
                    y: rows as i16,
                },
            )
        };
        if status != S_OK {
            return Err(io::Error::other(format!(
                "ResizePseudoConsole failed with status 0x{status:x}"
            )));
        }
        Ok(())
    }

    pub(super) fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read = 0u32;
        let ok = unsafe { ReadFile(self.cmd_out, buf.as_mut_ptr(), buf.len() as u32, &mut read, null_mut()) };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            if err == ERROR_BROKEN_PIPE {
                return Ok(0);
            }
            return Err(io::Error::from_raw_os_error(err as i32));
        }
        Ok(read as usize)
    }

    pub(super) fn write_all(&self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            let mut written = 0u32;
            let ok = unsafe { WriteFile(self.cmd_in, buf.as_ptr(), buf.len() as u32, &mut written, null_mut()) };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                if err == ERROR_BROKEN_PIPE {
                    return Ok(());
                }
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            buf = &buf[written as usize..];
        }
        Ok(())
    }

    pub(super) fn wait(&self) -> io::Result<u32> {
        let status = unsafe { WaitForSingleObject(self.process, INFINITE) };
        if status == WAIT_TIMEOUT {
            return Ok(STILL_ACTIVE);
        }
        let mut exit_code = STILL_ACTIVE;
        let ok = unsafe { GetExitCodeProcess(self.process, &mut exit_code) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(exit_code)
    }

    pub(super) fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }

        unsafe {
            close_pseudo_console(self.hpc);
            close_handle(self.pty_in);
            close_handle(self.pty_out);
            close_handle(self.cmd_in);
            close_handle(self.cmd_out);
            close_handle(self.process_thread);
            close_handle(self.process);
        }
    }
}

impl Drop for ConPtyProcess {
    fn drop(&mut self) {
        self.close();
    }
}

fn create_process_attached_to_pty(hpc: Hpcon, command_line: &str) -> io::Result<PROCESS_INFORMATION> {
    let mut size = 0usize;
    unsafe {
        let _ = InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size);
    }
    let mut attr_list = vec![0u8; size];
    let attr_ptr = attr_list.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
    if unsafe { InitializeProcThreadAttributeList(attr_ptr, 1, 0, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let result = (|| {
        // The Windows child is attached to the pseudo console through the extended startup
        // attribute list; there is no stdin/stdout PTY fd model like on Unix.
        if unsafe {
            UpdateProcThreadAttribute(
                attr_ptr,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                hpc as *mut c_void,
                size_of::<Hpcon>(),
                null_mut(),
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }

        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = attr_ptr;

        let mut command_line = wide(command_line);
        let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };
        if unsafe {
            CreateProcessW(
                null(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                0,
                EXTENDED_STARTUPINFO_PRESENT,
                null(),
                null(),
                &mut startup.StartupInfo,
                &mut pi,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }

        Ok(pi)
    })();

    unsafe {
        if !attr_ptr.is_null() {
            DeleteProcThreadAttributeList(attr_ptr);
        }
    }
    result
}

fn create_pseudo_console(coord: Coord, pty_in: HANDLE, pty_out: HANDLE) -> io::Result<Hpcon> {
    let create = create_pseudo_console_ptr()?;
    let mut hpc: Hpcon = null_mut();
    let status = unsafe { create(coord, pty_in, pty_out, 0, &mut hpc) };
    if status != S_OK {
        return Err(io::Error::other(format!(
            "CreatePseudoConsole failed with status 0x{status:x}"
        )));
    }
    Ok(hpc)
}

unsafe fn close_pseudo_console(hpc: Hpcon) {
    if hpc.is_null() {
        return;
    }
    if let Ok(close) = close_pseudo_console_ptr() {
        unsafe { close(hpc) };
    }
}

unsafe fn close_handle(handle: HANDLE) {
    if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
        let _ = unsafe { CloseHandle(handle) };
    }
}

fn is_conpty_available() -> bool {
    create_pseudo_console_ptr().is_ok()
        && resize_pseudo_console_ptr().is_ok()
        && close_pseudo_console_ptr().is_ok()
}

fn create_pseudo_console_ptr() -> io::Result<CreatePseudoConsoleFn> {
    proc_address("CreatePseudoConsole")
}

fn resize_pseudo_console_ptr() -> io::Result<ResizePseudoConsoleFn> {
    proc_address("ResizePseudoConsole")
}

fn close_pseudo_console_ptr() -> io::Result<ClosePseudoConsoleFn> {
    proc_address("ClosePseudoConsole")
}

fn proc_address<T>(name: &str) -> io::Result<T> {
    let kernel32 = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) };
    if kernel32.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut symbol_name = name.as_bytes().to_vec();
    symbol_name.push(0);
    let symbol = unsafe { GetProcAddress(kernel32, symbol_name.as_ptr()) };
    if symbol.is_none() {
        return Err(io::Error::other(format!("{name} not found")));
    }
    let symbol = symbol.expect("checked FARPROC");
    Ok(unsafe { std::mem::transmute_copy(&symbol) })
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
