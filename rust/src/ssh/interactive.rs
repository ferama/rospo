use std::time::Duration;

use russh::ChannelMsg;
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc;

#[cfg(unix)]
use nix::sys::termios::{self, SetArg, Termios};

pub(crate) async fn drain_channel(
    channel: &mut russh::Channel<russh::client::Msg>,
    interactive: bool,
) -> Result<u32, String> {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let mut stdin_closed = false;
    let mut exit_status = None::<u32>;
    #[allow(unused_mut)]
    let mut terminal_guard = if interactive {
        TerminalModeGuard::activate().ok()
    } else {
        None
    };
    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Option<Vec<u8>>>();
    let _stdin_thread = if interactive {
        Some(spawn_stdin_reader(stdin_tx))
    } else {
        None
    };
    let mut resize_interval = tokio::time::interval(Duration::from_millis(100));
    let mut last_size = if interactive { terminal_size().ok() } else { None };

    loop {
        tokio::select! {
            stdin_msg = stdin_rx.recv(), if interactive && !stdin_closed => {
                match stdin_msg {
                    Some(Some(bytes)) => channel.data(bytes.as_slice()).await.map_err(|err| err.to_string())?,
                    Some(None) | None => {
                        stdin_closed = true;
                        let _ = channel.eof().await;
                    }
                }
            }
            msg = channel.wait() => {
                let Some(msg) = msg else {
                    break;
                };
                match msg {
                    ChannelMsg::Data { data } => {
                        stdout.write_all(&data).await.map_err(|err| err.to_string())?;
                        stdout.flush().await.map_err(|err| err.to_string())?;
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        stderr.write_all(&data).await.map_err(|err| err.to_string())?;
                        stderr.flush().await.map_err(|err| err.to_string())?;
                    }
                    ChannelMsg::ExitStatus { exit_status: code } => {
                        exit_status = Some(code);
                        if interactive && !stdin_closed {
                            let _ = channel.eof().await;
                        }
                        break;
                    }
                    ChannelMsg::Eof => {}
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            _ = resize_interval.tick(), if interactive && terminal_guard.is_some() => {
                if let Ok((cols, rows)) = terminal_size() {
                    if last_size != Some((cols, rows)) {
                        last_size = Some((cols, rows));
                        let _ = channel.window_change(cols, rows, 0, 0).await;
                    }
                }
            }
        }
    }

    drop(terminal_guard.take());

    if let Some(code) = exit_status {
        Ok(code)
    } else {
        Err("channel closed without exit status".to_string())
    }
}

fn spawn_stdin_reader(stdin_tx: mpsc::UnboundedSender<Option<Vec<u8>>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match std::io::Read::read(&mut stdin, &mut buf) {
                Ok(0) => {
                    let _ = stdin_tx.send(None);
                    break;
                }
                Ok(n) => {
                    if stdin_tx.send(Some(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = stdin_tx.send(None);
                    break;
                }
            }
        }
    })
}

#[cfg(unix)]
struct TerminalModeGuard {
    original: Termios,
}

#[cfg(unix)]
impl TerminalModeGuard {
    fn activate() -> Result<Self, String> {
        if unsafe { nix::libc::isatty(0) } != 1 {
            return Err("stdin is not a terminal".to_string());
        }
        let mut term = termios::tcgetattr(std::io::stdin()).map_err(|err| err.to_string())?;
        let original = term.clone();
        termios::cfmakeraw(&mut term);
        termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &term).map_err(|err| err.to_string())?;
        Ok(Self { original })
    }
}

#[cfg(unix)]
impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &self.original);
    }
}

#[cfg(not(unix))]
struct TerminalModeGuard;

#[cfg(not(unix))]
impl TerminalModeGuard {
    fn activate() -> Result<Self, String> {
        Err("raw terminal mode not implemented".to_string())
    }
}

#[cfg(unix)]
pub(crate) fn terminal_size() -> Result<(u32, u32), String> {
    use std::mem::zeroed;

    let mut winsize: nix::libc::winsize = unsafe { zeroed() };
    let result = unsafe { nix::libc::ioctl(0, nix::libc::TIOCGWINSZ as _, &mut winsize) };
    if result == -1 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok((u32::from(winsize.ws_col), u32::from(winsize.ws_row)))
}

#[cfg(not(unix))]
pub(crate) fn terminal_size() -> Result<(u32, u32), String> {
    Err("terminal size not implemented".to_string())
}
