use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{self, Duration, Instant};

use crate::logging;
use crate::sftp::ProgressReporter;
use crate::utils::byte_count_si;

#[derive(Clone)]
pub(crate) struct ProgressManager {
    direction: &'static str,
    enabled: bool,
    state: Arc<Mutex<ProgressState>>,
    renderer: Arc<Mutex<Option<JoinHandle<()>>>>,
}

struct ProgressState {
    bars: Vec<ProgressBar>,
    rendered_lines: usize,
    closed: bool,
}

#[derive(Clone)]
struct ProgressBar {
    direction: &'static str,
    file_name: String,
    file_size: u64,
    current: u64,
    started_at: Instant,
    done: bool,
}

impl ProgressManager {
    pub(crate) fn download() -> Self {
        Self::new("⬇")
    }

    pub(crate) fn upload() -> Self {
        Self::new("⬆")
    }

    fn new(direction: &'static str) -> Self {
        Self {
            direction,
            enabled: io::stdout().is_terminal(),
            state: Arc::new(Mutex::new(ProgressState {
                bars: Vec::new(),
                rendered_lines: 0,
                closed: false,
            })),
            renderer: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) async fn finish(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        {
            let mut state = self.state.lock().expect("progress state mutex poisoned");
            state.closed = true;
        }
        if let Some(handle) = self
            .renderer
            .lock()
            .expect("progress renderer mutex poisoned")
            .take()
        {
            handle.await.map_err(|err| err.to_string())?;
        }
        logging::set_terminal_overlay(None);
        Ok(())
    }

    async fn ensure_renderer(&self) {
        if !self.enabled {
            return;
        }
        let mut guard = self
            .renderer
            .lock()
            .expect("progress renderer mutex poisoned");
        if guard.is_some() {
            return;
        }
        let state = self.state.clone();
        logging::set_terminal_overlay(Some(Arc::new({
            let state = state.clone();
            move || {
                let mut out = io::stdout().lock();
                let _ = clear_snapshot_locked(&state, &mut out);
                let _ = out.flush();
            }
        })));
        *guard = Some(tokio::spawn(async move {
            // A single renderer owns terminal repainting for all transfer bars so concurrent file
            // workers do not interleave escape sequences and corrupt stdout.
            let mut ticker = time::interval(Duration::from_millis(100));
            loop {
                ticker.tick().await;
                if render_snapshot(&state).is_err() {
                    break;
                }
                let done = {
                    let state = state.lock().expect("progress state mutex poisoned");
                    state.closed && state.bars.iter().all(|bar| bar.done)
                };
                if done {
                    let _ = clear_snapshot_sync(&state);
                    break;
                }
            }
        }));
    }
}

impl ProgressReporter for ProgressManager {
    fn spawn(
        &self,
        file_size: u64,
        offset: u64,
        file_name: String,
        mut progress_rx: mpsc::Receiver<u64>,
    ) -> JoinHandle<()> {
        let state = self.state.clone();
        let manager = self.clone();
        tokio::spawn(async move {
            if !manager.enabled {
                while progress_rx.recv().await.is_some() {}
                return;
            }
            manager.ensure_renderer().await;
            let id = {
                let mut state = state.lock().expect("progress state mutex poisoned");
                state.bars.push(ProgressBar {
                    direction: manager.direction,
                    file_name,
                    file_size,
                    current: offset,
                    started_at: Instant::now(),
                    done: false,
                });
                state.bars.len() - 1
            };
            while let Some(delta) = progress_rx.recv().await {
                let mut state = state.lock().expect("progress state mutex poisoned");
                if let Some(bar) = state.bars.get_mut(id) {
                    bar.current = bar.current.saturating_add(delta).min(bar.file_size);
                }
            }
            let mut state = state.lock().expect("progress state mutex poisoned");
            if let Some(bar) = state.bars.get_mut(id) {
                bar.done = true;
                bar.current = bar.file_size;
            }
        })
    }
}

fn render_snapshot(state: &Arc<Mutex<ProgressState>>) -> io::Result<()> {
    let (lines, previous) = {
        let mut state = state.lock().expect("progress state mutex poisoned");
        let lines = state
            .bars
            .iter()
            .filter(|bar| !bar.done)
            .map(format_bar_line)
            .collect::<Vec<_>>();
        let previous = state.rendered_lines;
        state.rendered_lines = lines.len();
        (lines, previous)
    };

    logging::with_output_lock(|| -> io::Result<()> {
        let mut out = io::stdout().lock();
        if previous > 0 {
            write!(out, "\x1b[{}F", previous)?;
        }
        // Keep the active bars as a terminal overlay instead of append-only output so reconnect
        // logs do not leave stale copies of the same bar behind.
        let total = previous.max(lines.len());
        for idx in 0..total {
            write!(out, "\x1b[2K")?;
            if let Some(line) = lines.get(idx) {
                writeln!(out, "{line}")?;
            } else {
                writeln!(out)?;
            }
        }
        out.flush()
    })
}

fn clear_snapshot_sync(state: &Arc<Mutex<ProgressState>>) -> io::Result<()> {
    logging::with_output_lock(|| -> io::Result<()> {
        let mut out = io::stdout().lock();
        clear_snapshot_locked(state, &mut out)?;
        out.flush()
    })
}

fn clear_snapshot_locked(state: &Arc<Mutex<ProgressState>>, out: &mut impl Write) -> io::Result<()> {
    let previous = {
        let mut state = state.lock().expect("progress state mutex poisoned");
        let previous = state.rendered_lines;
        state.rendered_lines = 0;
        previous
    };
    if previous == 0 {
        return Ok(());
    }
    write!(out, "\x1b[{}F", previous)?;
    for _ in 0..previous {
        write!(out, "\x1b[2K\n")?;
    }
    Ok(())
}

fn format_bar_line(bar: &ProgressBar) -> String {
    let elapsed = bar.started_at.elapsed().as_secs_f64().max(0.001);
    let speed = byte_count_si((bar.current as f64 / elapsed) as i64);
    let pct = if bar.file_size == 0 {
        100.0
    } else {
        (bar.current as f64 / bar.file_size as f64) * 100.0
    };
    let bar_fill = format_bar_fill(bar.current, bar.file_size, 50);
    format!(
        "{} {} {:>5.1}s ({speed}/s) {:>6.2}% [{}] {} / {}",
        bar.direction,
        bar.file_name,
        elapsed,
        pct,
        bar_fill,
        byte_count_si(bar.current as i64),
        byte_count_si(bar.file_size as i64),
    )
}

fn format_bar_fill(current: u64, total: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if total == 0 {
        return "=".repeat(width);
    }
    let ratio = (current as f64 / total as f64).clamp(0.0, 1.0);
    let filled = ((ratio * width as f64).round() as usize).min(width);
    if filled == 0 {
        return "-".repeat(width);
    }
    if filled >= width {
        return "=".repeat(width);
    }
    let mut out = String::with_capacity(width);
    out.push_str(&"=".repeat(filled.saturating_sub(1)));
    out.push('>');
    out.push_str(&"-".repeat(width - filled));
    out
}
