use std::path::{Path, PathBuf};

use futures::stream::{FuturesUnordered, StreamExt};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::ssh::{ClientOptions, Session};

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
    session: Session,
    sftp: SftpSession,
}

impl Client {
    pub async fn connect(options: ClientOptions) -> Result<Self, String> {
        let mut session = Session::connect(options).await?;
        let sftp = session.open_sftp().await?;
        Ok(Self { session, sftp })
    }

    pub async fn close(&mut self) -> Result<(), String> {
        let _ = self.sftp.close().await;
        self.session.disconnect().await
    }

    pub async fn get_file(&self, remote: &str, local: &str) -> Result<(), String> {
        self.get_file_with_options(remote, local, TransferOptions::new(1, 1))
            .await
    }

    pub async fn put_file(&self, remote: &str, local: &str) -> Result<(), String> {
        self.put_file_with_options(remote, local, TransferOptions::new(1, 1))
            .await
    }

    pub async fn get_file_with_options(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
    ) -> Result<(), String> {
        let remote_path = canonicalize_or(remote, ".", &self.sftp).await?;
        let remote_meta = self
            .sftp
            .metadata(remote_path.clone())
            .await
            .map_err(|err| err.to_string())?;
        let local_path = resolve_local_target(local, &remote_path).await?;
        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent).await.map_err(|err| err.to_string())?;
        }

        let offset = match fs::metadata(&local_path).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        let size = remote_meta.len();
        if offset >= size {
            return Ok(());
        }

        if options.max_workers <= 1 || size.saturating_sub(offset) as usize <= DEFAULT_CHUNK_SIZE {
            return self.copy_file_download(&remote_path, &local_path, offset).await;
        }

        self.download_chunks(&remote_path, &local_path, offset, size, options.max_workers)
            .await
    }

    pub async fn put_file_with_options(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
    ) -> Result<(), String> {
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if local_meta.is_dir() {
            return Err(format!("local path is not a file: {local}"));
        }

        let remote_target = resolve_remote_target(remote, local, &self.sftp).await?;
        let offset = match self.sftp.metadata(remote_target.clone()).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        let size = local_meta.len();
        if offset >= size {
            return Ok(());
        }

        if let Some(parent) = remote_parent(&remote_target) {
            ensure_remote_dir(&self.sftp, &parent).await?;
        }

        if options.max_workers <= 1 || size.saturating_sub(offset) as usize <= DEFAULT_CHUNK_SIZE {
            return self.copy_file_upload(&remote_target, local, offset).await;
        }

        self.upload_chunks(&remote_target, local, offset, size, options.max_workers)
            .await
    }

    pub async fn get_recursive(&self, remote: &str, local: &str) -> Result<(), String> {
        self.get_recursive_with_options(remote, local, TransferOptions::new(1, 1))
            .await
    }

    pub async fn put_recursive(&self, remote: &str, local: &str) -> Result<(), String> {
        self.put_recursive_with_options(remote, local, TransferOptions::new(1, 1))
            .await
    }

    pub async fn get_recursive_with_options(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
    ) -> Result<(), String> {
        let remote_root = canonicalize_or(remote, ".", &self.sftp).await?;
        let remote_meta = self
            .sftp
            .metadata(remote_root.clone())
            .await
            .map_err(|err| err.to_string())?;
        if !remote_meta.is_dir() {
            return Err(format!("remote path is not a directory: {remote_root}"));
        }
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if !local_meta.is_dir() {
            return Err(format!("local path is not a directory: {local}"));
        }

        let mut jobs = Vec::new();
        self.collect_remote_jobs(&remote_root, Path::new(local), &remote_root, &mut jobs)
            .await?;
        self.run_download_jobs(jobs, options).await
    }

    pub async fn put_recursive_with_options(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
    ) -> Result<(), String> {
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if !local_meta.is_dir() {
            return Err(format!("local path is not a directory: {local}"));
        }

        let remote_root = canonicalize_or(remote, ".", &self.sftp).await?;
        let remote_meta = self
            .sftp
            .metadata(remote_root.clone())
            .await
            .map_err(|err| err.to_string())?;
        if !remote_meta.is_dir() {
            return Err(format!("remote path is not a directory: {remote_root}"));
        }

        let base = Path::new(local)
            .file_name()
            .ok_or_else(|| format!("invalid local path: {local}"))?
            .to_string_lossy()
            .into_owned();
        let target_root = remote_join(&remote_root, &base);
        ensure_remote_dir(&self.sftp, &target_root).await?;

        let mut jobs = Vec::new();
        self.collect_local_jobs(Path::new(local), &target_root, &mut jobs).await?;
        self.run_upload_jobs(jobs, options).await
    }

    async fn copy_file_download(&self, remote_path: &str, local_path: &str, offset: u64) -> Result<(), String> {
        let mut remote_file = self
            .sftp
            .open(remote_path.to_string())
            .await
            .map_err(|err| err.to_string())?;
        let mut local_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(local_path)
            .await
            .map_err(|err| err.to_string())?;

        if offset > 0 {
            remote_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
            local_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
        }

        tokio::io::copy(&mut remote_file, &mut local_file)
            .await
            .map_err(|err| err.to_string())?;
        local_file.flush().await.map_err(|err| err.to_string())?;
        remote_file.shutdown().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn copy_file_upload(&self, remote_target: &str, local: &str, offset: u64) -> Result<(), String> {
        let mut local_file = fs::File::open(local).await.map_err(|err| err.to_string())?;
        let mut remote_file = self
            .sftp
            .open_with_flags(
                remote_target.to_string(),
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::READ,
            )
            .await
            .map_err(|err| err.to_string())?;

        if offset > 0 {
            local_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
            remote_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
        }

        tokio::io::copy(&mut local_file, &mut remote_file)
            .await
            .map_err(|err| err.to_string())?;
        remote_file.flush().await.map_err(|err| err.to_string())?;
        remote_file.shutdown().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn download_chunks(
        &self,
        remote_path: &str,
        local_path: &str,
        offset: u64,
        size: u64,
        max_workers: usize,
    ) -> Result<(), String> {
        let path = PathBuf::from(local_path);
        let std_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(&path)
            .map_err(|err| err.to_string())?;
        std_file.set_len(size).map_err(|err| err.to_string())?;

        let mut chunks = build_chunks(offset, size);
        let mut in_flight = FuturesUnordered::new();
        let limit = max_workers.max(1);

        while !chunks.is_empty() || !in_flight.is_empty() {
            while in_flight.len() < limit && !chunks.is_empty() {
                let chunk = chunks.remove(0);
                let remote = remote_path.to_string();
                let local = path.clone();
                in_flight.push(self.download_chunk(remote, local, chunk.offset, chunk.len));
            }
            match in_flight.next().await {
                Some(result) => result?,
                None => break,
            }
        }

        Ok(())
    }

    async fn upload_chunks(
        &self,
        remote_target: &str,
        local_path: &str,
        offset: u64,
        size: u64,
        max_workers: usize,
    ) -> Result<(), String> {
        let remote = remote_target.to_string();
        let mut init_file = self
            .sftp
            .open_with_flags(remote.clone(), OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::READ)
            .await
            .map_err(|err| err.to_string())?;
        init_file.shutdown().await.map_err(|err| err.to_string())?;

        let local_path = PathBuf::from(local_path);

        let mut chunks = build_chunks(offset, size);
        let mut in_flight = FuturesUnordered::new();
        let limit = max_workers.max(1);

        while !chunks.is_empty() || !in_flight.is_empty() {
            while in_flight.len() < limit && !chunks.is_empty() {
                let chunk = chunks.remove(0);
                let remote_clone = remote.clone();
                let local_clone = local_path.clone();
                in_flight.push(self.upload_chunk(remote_clone, local_clone, chunk.offset, chunk.len));
            }
            match in_flight.next().await {
                Some(result) => result?,
                None => break,
            }
        }

        Ok(())
    }

    async fn download_chunk(
        &self,
        remote_path: String,
        local_path: PathBuf,
        offset: u64,
        len: usize,
    ) -> Result<(), String> {
        let mut remote_file = self.sftp.open(remote_path).await.map_err(|err| err.to_string())?;
        remote_file
            .seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|err| err.to_string())?;

        let mut buf = vec![0u8; len];
        remote_file
            .read_exact(&mut buf)
            .await
            .map_err(|err| err.to_string())?;
        remote_file.shutdown().await.map_err(|err| err.to_string())?;

        let mut local = fs::OpenOptions::new()
            .write(true)
            .read(true)
            .open(local_path)
            .await
            .map_err(|err| err.to_string())?;
        local.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|err| err.to_string())?;
        local.write_all(&buf).await.map_err(|err| err.to_string())?;
        local.flush().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn upload_chunk(
        &self,
        remote_target: String,
        local_path: PathBuf,
        offset: u64,
        len: usize,
    ) -> Result<(), String> {
        let mut local = fs::File::open(local_path).await.map_err(|err| err.to_string())?;
        local.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|err| err.to_string())?;
        let mut buf = vec![0u8; len];
        local.read_exact(&mut buf).await.map_err(|err| err.to_string())?;

        let mut remote = self
            .sftp
            .open_with_flags(remote_target, OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::READ)
            .await
            .map_err(|err| err.to_string())?;
        remote
            .seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|err| err.to_string())?;
        remote.write_all(&buf).await.map_err(|err| err.to_string())?;
        remote.flush().await.map_err(|err| err.to_string())?;
        remote.shutdown().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn collect_remote_jobs(
        &self,
        remote_path: &str,
        local_root: &Path,
        root: &str,
        jobs: &mut Vec<DownloadJob>,
    ) -> Result<(), String> {
        let entries = self.sftp.read_dir(remote_path).await.map_err(|err| err.to_string())?;
        for entry in entries {
            let child_remote = remote_join(remote_path, &entry.file_name());
            let relative = child_remote.trim_start_matches(root).trim_start_matches('/');
            let local_path = local_root.join(relative);
            if entry.metadata().is_dir() {
                fs::create_dir_all(&local_path).await.map_err(|err| err.to_string())?;
                Box::pin(self.collect_remote_jobs(&child_remote, local_root, root, jobs)).await?;
            } else {
                if let Some(parent) = local_path.parent() {
                    fs::create_dir_all(parent).await.map_err(|err| err.to_string())?;
                }
                jobs.push(DownloadJob {
                    remote: child_remote,
                    local: local_path.to_string_lossy().into_owned(),
                });
            }
        }
        Ok(())
    }

    async fn collect_local_jobs(
        &self,
        local_path: &Path,
        remote_root: &str,
        jobs: &mut Vec<UploadJob>,
    ) -> Result<(), String> {
        let mut dir = fs::read_dir(local_path).await.map_err(|err| err.to_string())?;
        while let Some(entry) = dir.next_entry().await.map_err(|err| err.to_string())? {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let target = remote_join(remote_root, &name);
            let metadata = entry.metadata().await.map_err(|err| err.to_string())?;
            if metadata.is_dir() {
                ensure_remote_dir(&self.sftp, &target).await?;
                Box::pin(self.collect_local_jobs(&path, &target, jobs)).await?;
            } else {
                jobs.push(UploadJob {
                    remote: target,
                    local: path.to_string_lossy().into_owned(),
                });
            }
        }
        Ok(())
    }

    async fn run_download_jobs(
        &self,
        jobs: Vec<DownloadJob>,
        options: TransferOptions,
    ) -> Result<(), String> {
        let mut iter = jobs.into_iter();
        let mut in_flight = FuturesUnordered::new();
        let limit = options.concurrent_files.max(1);

        while !in_flight.is_empty() || iter.len() > 0 {
            while in_flight.len() < limit {
                let Some(job) = iter.next() else {
                    break;
                };
                let remote = job.remote;
                let local = job.local;
                in_flight.push(async move {
                    self.get_file_with_options(&remote, &local, TransferOptions::new(options.max_workers, 1))
                        .await
                });
            }
            match in_flight.next().await {
                Some(result) => result?,
                None => break,
            }
        }

        Ok(())
    }

    async fn run_upload_jobs(
        &self,
        jobs: Vec<UploadJob>,
        options: TransferOptions,
    ) -> Result<(), String> {
        let mut iter = jobs.into_iter();
        let mut in_flight = FuturesUnordered::new();
        let limit = options.concurrent_files.max(1);

        while !in_flight.is_empty() || iter.len() > 0 {
            while in_flight.len() < limit {
                let Some(job) = iter.next() else {
                    break;
                };
                let remote = job.remote;
                let local = job.local;
                in_flight.push(async move {
                    self.put_file_with_options(&remote, &local, TransferOptions::new(options.max_workers, 1))
                        .await
                });
            }
            match in_flight.next().await {
                Some(result) => result?,
                None => break,
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Chunk {
    offset: u64,
    len: usize,
}

#[derive(Debug, Clone)]
struct DownloadJob {
    remote: String,
    local: String,
}

#[derive(Debug, Clone)]
struct UploadJob {
    remote: String,
    local: String,
}

fn build_chunks(offset: u64, size: u64) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut start = offset;
    while start < size {
        let remaining = size - start;
        let len = remaining.min(DEFAULT_CHUNK_SIZE as u64) as usize;
        chunks.push(Chunk { offset: start, len });
        start += len as u64;
    }
    chunks
}

fn remote_join(base: &str, child: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{child}")
    } else if base.is_empty() {
        child.to_string()
    } else {
        format!("{base}/{child}")
    }
}

fn remote_parent(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(parent, _)| {
        if parent.is_empty() {
            "/".to_string()
        } else {
            parent.to_string()
        }
    })
}

async fn canonicalize_or(path: &str, fallback: &str, sftp: &SftpSession) -> Result<String, String> {
    let target = if path.is_empty() { fallback } else { path };
    sftp.canonicalize(target).await.map_err(|err| err.to_string())
}

async fn resolve_local_target(local: &str, remote_path: &str) -> Result<String, String> {
    let local_path = if local.is_empty() { "." } else { local };
    match fs::metadata(local_path).await {
        Ok(meta) if meta.is_dir() => {
            let file_name = Path::new(remote_path)
                .file_name()
                .ok_or_else(|| format!("invalid remote path: {remote_path}"))?;
            Ok(Path::new(local_path).join(file_name).to_string_lossy().into_owned())
        }
        _ => Ok(local_path.to_string()),
    }
}

async fn resolve_remote_target(remote: &str, local: &str, sftp: &SftpSession) -> Result<String, String> {
    let remote_path = if remote.is_empty() { "." } else { remote };
    if let Ok(meta) = sftp.metadata(remote_path).await {
        if meta.is_dir() {
            let file_name = Path::new(local)
                .file_name()
                .ok_or_else(|| format!("invalid local path: {local}"))?;
            return Ok(remote_join(remote_path, &file_name.to_string_lossy()));
        }
        return Ok(remote_path.to_string());
    }

    if remote.is_empty() {
        let file_name = Path::new(local)
            .file_name()
            .ok_or_else(|| format!("invalid local path: {local}"))?;
        Ok(file_name.to_string_lossy().into_owned())
    } else {
        Ok(remote.to_string())
    }
}

async fn ensure_remote_dir(sftp: &SftpSession, path: &str) -> Result<(), String> {
    if path.is_empty() || path == "." || path == "/" {
        return Ok(());
    }
    if sftp.try_exists(path).await.map_err(|err| err.to_string())? {
        return Ok(());
    }
    if let Some(parent) = remote_parent(path) {
        Box::pin(ensure_remote_dir(sftp, &parent)).await?;
    }
    match sftp.create_dir(path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            if sftp.try_exists(path).await.map_err(|inner| inner.to_string())? {
                Ok(())
            } else {
                Err(err.to_string())
            }
        }
    }
}
