use russh_sftp::client::SftpSession;

use crate::ssh::Session;

pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024;
pub const DEFAULT_MAX_WORKERS: usize = 12;
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

pub struct Client {
    pub(crate) session: Session,
    pub(crate) sftp: SftpSession,
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
