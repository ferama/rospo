use std::ffi::{OsString, c_void};
use std::io;
use std::ptr::null_mut;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;

use windows_sys::Win32::Foundation::{
    ERROR_CALL_NOT_IMPLEMENTED, ERROR_FAILED_SERVICE_CONTROLLER_CONNECT, NO_ERROR,
};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP,
    SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP,
    SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS, SERVICE_STATUS_HANDLE,
    SERVICE_STOP_PENDING, SERVICE_STOPPED, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
    SetServiceStatus, StartServiceCtrlDispatcherW,
};

const SERVICE_NAME: &str = "rospo";

static SERVICE_ARGS: OnceLock<Vec<OsString>> = OnceLock::new();
static SERVICE_EVENTS: Mutex<Option<mpsc::Sender<ServiceEvent>>> = Mutex::new(None);

#[derive(Debug, Clone, Copy)]
enum ServiceEvent {
    Stop,
    WorkerExit(u32),
}

pub fn try_run(args: Vec<OsString>) -> Result<bool, String> {
    let _ = SERVICE_ARGS.set(args);
    let mut service_name = wide(SERVICE_NAME);
    let mut table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: service_name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: null_mut(),
            lpServiceProc: None,
        },
    ];

    let started = unsafe { StartServiceCtrlDispatcherW(table.as_mut_ptr()) };
    if started == 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(ERROR_FAILED_SERVICE_CONTROLLER_CONNECT as i32) {
            return Ok(false);
        }
        return Err(err.to_string());
    }

    Ok(true)
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    if let Err(err) = run_service() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run_service() -> Result<(), String> {
    let service_name = wide(SERVICE_NAME);
    let (tx, rx) = mpsc::channel::<ServiceEvent>();
    {
        let mut guard = SERVICE_EVENTS.lock().map_err(|_| "service event mutex poisoned".to_string())?;
        *guard = Some(tx.clone());
    }

    let status_handle = unsafe {
        RegisterServiceCtrlHandlerExW(service_name.as_ptr(), Some(service_control_handler), null_mut())
    };
    if status_handle.is_null() {
        return Err(io::Error::last_os_error().to_string());
    }

    set_status(status_handle, SERVICE_START_PENDING, 0, 0)?;
    set_status(
        status_handle,
        SERVICE_RUNNING,
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
        0,
    )?;

    let worker_args = SERVICE_ARGS
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::args_os().collect());
    thread::spawn(move || {
        let code = crate::cli::run(worker_args);
        let _ = tx.send(ServiceEvent::WorkerExit(code as u32));
    });

    let exit_code = match rx.recv() {
        Ok(ServiceEvent::Stop) => 0,
        Ok(ServiceEvent::WorkerExit(code)) => code,
        Err(_) => 1,
    };

    set_status(status_handle, SERVICE_STOP_PENDING, 0, 0)?;
    set_status(status_handle, SERVICE_STOPPED, 0, exit_code)?;
    std::process::exit(exit_code as i32);
}

unsafe extern "system" fn service_control_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut c_void,
    _context: *mut c_void,
) -> u32 {
    match control {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            if let Ok(guard) = SERVICE_EVENTS.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(ServiceEvent::Stop);
                }
            }
            NO_ERROR
        }
        SERVICE_CONTROL_INTERROGATE => NO_ERROR,
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

fn set_status(
    handle: SERVICE_STATUS_HANDLE,
    current_state: u32,
    controls_accepted: u32,
    exit_code: u32,
) -> Result<(), String> {
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: current_state,
        dwControlsAccepted: controls_accepted,
        dwWin32ExitCode: exit_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };
    let result = unsafe { SetServiceStatus(handle, &status) };
    if result == 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(())
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
