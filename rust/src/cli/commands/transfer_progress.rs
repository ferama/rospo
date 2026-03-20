use std::io::{self, IsTerminal, Write};
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration, Instant};

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
            let mut state = self.state.lock().await;
            state.closed = true;
        }
        if let Some(handle) = self.renderer.lock().await.take() {
            handle.await.map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    async fn ensure_renderer(&self) {
        if !self.enabled {
            return;
        }
        let mut guard = self.renderer.lock().await;
        if guard.is_some() {
            return;
        }
        let state = self.state.clone();
        *guard = Some(tokio::spawn(async move {
            // A single renderer owns terminal repainting for all transfer bars so concurrent file
            // workers do not interleave escape sequences and corrupt stdout.
            let mut ticker = time::interval(Duration::from_millis(100));
            loop {
                ticker.tick().await;
                if render_snapshot(&state).await.is_err() {
                    break;
                }
                let done = {
                    let state = state.lock().await;
                    state.closed && state.bars.iter().all(|bar| bar.done)
                };
                if done {
                    let _ = clear_snapshot(&state).await;
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
                let mut state = state.lock().await;
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
                let mut state = state.lock().await;
                if let Some(bar) = state.bars.get_mut(id) {
                    bar.current = bar.current.saturating_add(delta).min(bar.file_size);
                }
            }
            let mut state = state.lock().await;
            if let Some(bar) = state.bars.get_mut(id) {
                bar.done = true;
                bar.current = bar.file_size;
            }
        })
    }
}

async fn render_snapshot(state: &Arc<Mutex<ProgressState>>) -> io::Result<()> {
    let (lines, previous) = {
        let mut state = state.lock().await;
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

    let mut out = io::stdout().lock();
    if previous > 0 {
        write!(out, "\x1b[{}F", previous)?;
    }
    // Redraw in place instead of printing append-only updates so interactive transfers behave like
    // the Go progress bars, while non-terminal stdout stays clean because rendering is disabled.
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
}

async fn clear_snapshot(state: &Arc<Mutex<ProgressState>>) -> io::Result<()> {
    let previous = {
        let mut state = state.lock().await;
        let previous = state.rendered_lines;
        state.rendered_lines = 0;
        previous
    };
    if previous == 0 {
        return Ok(());
    }
    let mut out = io::stdout().lock();
    write!(out, "\x1b[{}F", previous)?;
    for _ in 0..previous {
        write!(out, "\x1b[2K\n")?;
    }
    out.flush()
}

fn format_bar_line(bar: &ProgressBar) -> String {
    let elapsed = bar.started_at.elapsed().as_secs_f64().max(0.001);
    let speed = byte_count_si((bar.current as f64 / elapsed) as i64);
    let pct = if bar.file_size == 0 {
        100.0
    } else {
        (bar.current as f64 / bar.file_size as f64) * 100.0
    };
    format!(
        "{} {} {:>5.1}s ({speed}/s) {:>6.2}% {} / {}",
        bar.direction,
        bar.file_name,
        elapsed,
        pct,
        byte_count_si(bar.current as i64),
        byte_count_si(bar.file_size as i64),
    )
}
