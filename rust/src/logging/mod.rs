use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use time::macros::format_description;
use time::OffsetDateTime;

pub const RED: &str = "\u{1b}[0;31m";
pub const GREEN: &str = "\u{1b}[0;32m";
pub const YELLOW: &str = "\u{1b}[0;33m";
pub const BLUE: &str = "\u{1b}[0;34m";
pub const MAGENTA: &str = "\u{1b}[0;35m";
pub const CYAN: &str = "\u{1b}[0;36m";
pub const WHITE: &str = "\u{1b}[0;37m";
const RESET: &str = "\u{1b}[0m";

static QUIET: AtomicBool = AtomicBool::new(false);
type TerminalOverlay = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Clone, Copy)]
pub struct Logger {
    prefix: &'static str,
    color: &'static str,
}

impl Logger {
    pub const fn new(prefix: &'static str, color: &'static str) -> Self {
        Self { prefix, color }
    }

    pub fn log(&self, args: fmt::Arguments<'_>) {
        if is_quiet() {
            return;
        }

        let timestamp = current_timestamp();
        with_output_lock(|| {
            suspend_terminal_overlay();
            let mut stdout = io::stdout().lock();
            if stdout.is_terminal() && !cfg!(windows) {
                let _ = writeln!(
                    stdout,
                    "{}{}{}{} {}",
                    self.color, self.prefix, RESET, timestamp, args
                );
            } else {
                let _ = writeln!(stdout, "{}{} {}", self.prefix, timestamp, args);
            }
            let _ = stdout.flush();
        });
    }
}

pub fn init_logging(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

pub fn set_terminal_overlay(overlay: Option<TerminalOverlay>) {
    *terminal_overlay().lock().expect("terminal overlay mutex poisoned") = overlay;
}

pub fn with_output_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = output_lock().lock().expect("output lock mutex poisoned");
    f()
}

fn current_timestamp() -> String {
    let format = format_description!("[year]/[month]/[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&format)
        .unwrap_or_else(|_| "1970/01/01 00:00:00".to_string())
}

fn suspend_terminal_overlay() {
    let overlay = terminal_overlay()
        .lock()
        .expect("terminal overlay mutex poisoned")
        .clone();
    if let Some(overlay) = overlay {
        overlay();
    }
}

fn output_lock() -> &'static Mutex<()> {
    static OUTPUT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    OUTPUT_LOCK.get_or_init(|| Mutex::new(()))
}

fn terminal_overlay() -> &'static Mutex<Option<TerminalOverlay>> {
    static TERMINAL_OVERLAY: OnceLock<Mutex<Option<TerminalOverlay>>> = OnceLock::new();
    TERMINAL_OVERLAY.get_or_init(|| Mutex::new(None))
}
