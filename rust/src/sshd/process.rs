use std::collections::HashMap;
use std::io;

use russh::{server, ChannelId};
#[cfg(unix)]
use tokio::process::Child;
use tokio::process::{ChildStderr, ChildStdout, Command};
use tokio::sync::mpsc;

use crate::utils::{current_home_dir, current_username, get_user_default_shell};

use super::{ChannelIo, PtyHandle, PtyRequest, SharedState};
#[cfg(windows)]
use super::windows_pty::ConPtyProcess;

pub(super) async fn spawn_shell(
    channel: ChannelId,
    handle: server::Handle,
    state: SharedState,
    env: HashMap<String, String>,
    pty: Option<PtyRequest>,
    command: Option<String>,
) -> Result<(), russh::Error> {
    #[cfg(unix)]
    if let Some(pty) = pty {
        return spawn_pty_shell_unix(channel, handle, state, env, pty, command).await;
    }

    #[cfg(windows)]
    if let Some(pty) = pty {
        return spawn_pty_shell_windows(channel, handle, state, env, pty, command).await;
    }

    let mut cmd = build_command(&state.options.shell_executable, command);
    apply_default_env(&mut cmd, &env, &state.options.shell_executable);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(current_home_dir());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    if let Some(session) = state.channels.lock().await.get_mut(&channel) {
        session.io = Some(ChannelIo::Stream(stdin_tx));
    }

    if let Some(mut stdin) = stdin {
        tokio::spawn(async move {
            while let Some(bytes) = stdin_rx.recv().await {
                if tokio::io::AsyncWriteExt::write_all(&mut stdin, &bytes).await.is_err() {
                    break;
                }
                let _ = tokio::io::AsyncWriteExt::flush(&mut stdin).await;
            }
        });
    }

    if let Some(stdout) = stdout {
        tokio::spawn(copy_stdout(channel, handle.clone(), stdout));
    }
    if let Some(stderr) = stderr {
        tokio::spawn(copy_stderr(channel, handle.clone(), stderr));
    }

    tokio::spawn(async move {
        let status = match child.wait().await {
            Ok(status) => status.code().unwrap_or(1) as u32,
            Err(_) => 1,
        };
        let _ = handle.exit_status_request(channel, status).await;
        let _ = handle.eof(channel).await;
        let _ = handle.close(channel).await;
    });

    Ok(())
}

#[cfg(unix)]
async fn spawn_pty_shell_unix(
    channel: ChannelId,
    handle: server::Handle,
    state: SharedState,
    env: HashMap<String, String>,
    pty: PtyRequest,
    command: Option<String>,
) -> Result<(), russh::Error> {
    use std::fs::File as StdFile;
    use std::os::fd::AsRawFd;

    use nix::libc;
    use nix::pty::{openpty, Winsize};
    use nix::unistd::setsid;
    let pty_result = openpty(
        Some(&Winsize {
            ws_row: pty.rows as u16,
            ws_col: pty.cols as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }),
        None,
    )
    .map_err(|err| russh::Error::IO(io::Error::other(err.to_string())))?;

    let mut cmd = build_command(&state.options.shell_executable, command);
    apply_default_env(&mut cmd, &env, &state.options.shell_executable);
    cmd.current_dir(current_home_dir());

    let slave_fd = pty_result.slave.as_raw_fd();
    let slave_file: StdFile = pty_result.slave.into();
    let stdin_file = slave_file.try_clone().map_err(russh::Error::IO)?;
    let stdout_file = slave_file.try_clone().map_err(russh::Error::IO)?;
    cmd.stdin(std::process::Stdio::from(stdin_file))
        .stdout(std::process::Stdio::from(stdout_file))
        .stderr(std::process::Stdio::from(slave_file));
    unsafe {
        cmd.pre_exec(move || {
            setsid().map_err(|err| io::Error::other(err.to_string()))?;
            if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = cmd.spawn()?;
    let master_file: StdFile = pty_result.master.into();
    let writer_std = master_file.try_clone().map_err(russh::Error::IO)?;
    let resizer_std = master_file.try_clone().map_err(russh::Error::IO)?;
    let reader = tokio::fs::File::from_std(master_file);
    let writer = tokio::fs::File::from_std(writer_std);

    let (stdin_tx, stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, resize_rx) = mpsc::unbounded_channel::<(u32, u32)>();

    if let Some(session) = state.channels.lock().await.get_mut(&channel) {
        session.io = Some(ChannelIo::Pty(PtyHandle { stdin_tx, resize_tx }));
    }

    tokio::spawn(run_pty_writer(writer, stdin_rx));
    tokio::spawn(run_pty_reader(channel, handle.clone(), reader));
    tokio::spawn(run_pty_resizer(resizer_std, resize_rx));
    tokio::spawn(wait_for_child(channel, handle, child));

    Ok(())
}

#[cfg(windows)]
async fn spawn_pty_shell_windows(
    channel: ChannelId,
    handle: server::Handle,
    state: SharedState,
    _env: HashMap<String, String>,
    pty: PtyRequest,
    command: Option<String>,
) -> Result<(), russh::Error> {
    let runtime = tokio::runtime::Handle::current();
    let process = ConPtyProcess::spawn(
        &build_windows_pty_command_line(&state.options.shell_executable, command),
        pty.cols as u16,
        pty.rows as u16,
    )
    .map_err(russh::Error::IO)?;

    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, mut resize_rx) = mpsc::unbounded_channel::<(u32, u32)>();
    if let Some(session) = state.channels.lock().await.get_mut(&channel) {
        session.io = Some(ChannelIo::Pty(PtyHandle { stdin_tx, resize_tx }));
    }

    let reader_process = process.clone();
    let reader_handle = handle.clone();
    let reader_runtime = runtime.clone();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match reader_process.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let _ = reader_runtime.block_on(reader_handle.data(channel, data));
                }
                Err(_) => break,
            }
        }
    });

    let writer_process = process.clone();
    std::thread::spawn(move || {
        while let Some(bytes) = stdin_rx.blocking_recv() {
            if writer_process.write_all(&bytes).is_err() {
                break;
            }
        }
    });

    let resizer_process = process.clone();
    std::thread::spawn(move || {
        while let Some((cols, rows)) = resize_rx.blocking_recv() {
            let _ = resizer_process.resize(cols as u16, rows as u16);
        }
    });

    std::thread::spawn(move || {
        let status = process.wait().unwrap_or(1);
        process.close();
        let _ = runtime.block_on(async move {
            let _ = handle.exit_status_request(channel, status).await;
            let _ = handle.eof(channel).await;
            let _ = handle.close(channel).await;
        });
    });

    Ok(())
}

async fn copy_stdout(channel: ChannelId, handle: server::Handle, mut stdout: ChildStdout) {
    use tokio::io::AsyncReadExt;

    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match stdout.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = handle.data(channel, buf[..n].to_vec()).await;
            }
        }
    }
}

async fn copy_stderr(channel: ChannelId, handle: server::Handle, mut stderr: ChildStderr) {
    use tokio::io::AsyncReadExt;

    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match stderr.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = handle.extended_data(channel, 1, buf[..n].to_vec()).await;
            }
        }
    }
}

#[cfg(unix)]
async fn run_pty_reader(channel: ChannelId, handle: server::Handle, mut reader: tokio::fs::File) {
    use tokio::io::AsyncReadExt;

    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = handle.data(channel, buf[..n].to_vec()).await;
            }
        }
    }
}

#[cfg(unix)]
async fn run_pty_writer(
    mut writer: tokio::fs::File,
    mut stdin_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) {
    use tokio::io::AsyncWriteExt;

    while let Some(bytes) = stdin_rx.recv().await {
        if writer.write_all(&bytes).await.is_err() {
            break;
        }
        let _ = writer.flush().await;
    }
}

#[cfg(unix)]
async fn run_pty_resizer(
    pty_file: std::fs::File,
    mut resize_rx: mpsc::UnboundedReceiver<(u32, u32)>,
) {
    while let Some((cols, rows)) = resize_rx.recv().await {
        let _ = resize_pty(&pty_file, cols, rows);
    }
}

#[cfg(unix)]
async fn wait_for_child(channel: ChannelId, handle: server::Handle, mut child: Child) {
    let status = match child.wait().await {
        Ok(status) => status.code().unwrap_or(1) as u32,
        Err(_) => 1,
    };
    let _ = handle.exit_status_request(channel, status).await;
    let _ = handle.eof(channel).await;
    let _ = handle.close(channel).await;
}

#[cfg(unix)]
fn resize_pty(pty_file: &std::fs::File, cols: u32, rows: u32) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    let winsize = nix::libc::winsize {
        ws_row: rows as u16,
        ws_col: cols as u16,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let result = unsafe { nix::libc::ioctl(pty_file.as_raw_fd(), nix::libc::TIOCSWINSZ as _, &winsize) };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn build_command(shell_executable: &str, command: Option<String>) -> Command {
    if !shell_executable.trim().is_empty() {
        let mut parts = shell_executable.split_whitespace();
        let program = parts.next().unwrap_or(shell_executable);
        let mut cmd = Command::new(program);
        for arg in parts {
            cmd.arg(arg);
        }
        if let Some(command) = command {
            cmd.arg(command);
        }
        return cmd;
    }

    if cfg!(windows) {
        let mut cmd = Command::new("powershell.exe");
        if let Some(command) = command {
            cmd.arg(command);
        }
        return cmd;
    }

    let shell = get_user_default_shell(&current_username());
    let mut cmd = Command::new(shell);
    if let Some(command) = command {
        cmd.arg("-c").arg(command);
    }
    cmd
}

#[cfg(windows)]
fn build_windows_pty_command_line(shell_executable: &str, command: Option<String>) -> String {
    let mut args = if shell_executable.trim().is_empty() {
        vec!["powershell.exe".to_string()]
    } else {
        shell_executable
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    if let Some(command) = command {
        args.push(command);
    }
    args.into_iter()
        .map(|arg| quote_windows_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn quote_windows_arg(arg: &str) -> String {
    if !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let escaped = arg.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn apply_default_env(cmd: &mut Command, env: &HashMap<String, String>, shell_executable: &str) {
    cmd.env_clear();
    for (key, value) in env {
        cmd.env(key, value);
    }

    let shell = if shell_executable.trim().is_empty() {
        if cfg!(windows) {
            "powershell.exe".to_string()
        } else {
            get_user_default_shell(&current_username())
        }
    } else {
        shell_executable
            .split_whitespace()
            .next()
            .unwrap_or(shell_executable)
            .to_string()
    };
    let home = current_home_dir();
    let user = current_username();
    let term = std::env::var("TERM").unwrap_or_else(|_| "xterm".to_string());
    let path = std::env::var("PATH").unwrap_or_else(|_| {
        if cfg!(windows) {
            r"C:\Windows\system32;C:\Windows;C:\Windows\System32\Wbem".to_string()
        } else {
            "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin".to_string()
        }
    });
    cmd.env("TERM", term)
        .env("HOME", home)
        .env("USER", &user)
        .env("LOGNAME", user)
        .env("PATH", path)
        .env("SHELL", shell);
}
