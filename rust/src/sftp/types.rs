use std::sync::Arc;

use russh_sftp::client::SftpSession;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;

use crate::ssh::Session;

pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024;
pub const DEFAULT_DOWNLOAD_MAX_WORKERS: usize = 12;
pub const DEFAULT_UPLOAD_MAX_WORKERS: usize = 16;
pub const DEFAULT_MAX_WORKERS: usize = DEFAULT_DOWNLOAD_MAX_WORKERS;
pub const DEFAULT_CONCURRENT_DOWNLOADS: usize = 4;
pub const DEFAULT_CONCURRENT_UPLOADS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct TransferOptions {
    pub max_workers: usize,
    pub concurrent_files: usize,
}

impl TransferOptions {
    pub const fn new(max_workers: usize, concurrent_files: usize) -> Self {
        Self {
            max_workers,
            concurrent_files,
        }
    }
}

pub trait ProgressReporter: Send + Sync {
    fn spawn(
        &self,
        file_size: u64,
        offset: u64,
        file_name: String,
        progress_rx: mpsc::Receiver<u64>,
    ) -> JoinHandle<()>;
}

pub struct Client {
    pub(crate) options: crate::ssh::ClientOptions,
    pub(crate) session: Session,
    pub(crate) sftp: SftpSession,
    pub(crate) recovery: Arc<RecoveryCoordinator>,
}

pub(crate) struct RecoveredConnection {
    pub(crate) session: Mutex<Option<Session>>,
    pub(crate) sftp: SftpSession,
}

pub(crate) enum RecoveryState {
    Idle,
    Recovering,
    Ready(Arc<RecoveredConnection>),
}

pub(crate) struct RecoveryCoordinator {
    pub(crate) state: Mutex<RecoveryState>,
    pub(crate) notify: Notify,
}

impl RecoveryCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(RecoveryState::Idle),
            notify: Notify::new(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Chunk {
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

pub(crate) struct DownloadJob {
    pub(crate) remote: String,
    pub(crate) local: String,
}

pub(crate) struct UploadJob {
    pub(crate) local: String,
    pub(crate) remote: String,
}
