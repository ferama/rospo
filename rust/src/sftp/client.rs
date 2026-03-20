use std::path::Path;

use futures::stream::{FuturesUnordered, StreamExt};
use russh_sftp::protocol::OpenFlags;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::ssh::{ClientOptions, Session};

use super::paths::{
    build_chunks, canonicalize_or, ensure_remote_dir, remote_join, remote_parent, resolve_local_target,
    resolve_remote_target,
};
use super::types::{Client, DownloadJob, TransferOptions, UploadJob};

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

        if options.max_workers <= 1 || size.saturating_sub(offset) as usize <= super::DEFAULT_CHUNK_SIZE {
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

        if options.max_workers <= 1 || size.saturating_sub(offset) as usize <= super::DEFAULT_CHUNK_SIZE {
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
        let mut chunks = build_chunks(offset, size);
        let mut in_flight = FuturesUnordered::new();
        let limit = max_workers.max(1);

        while !chunks.is_empty() || !in_flight.is_empty() {
            while in_flight.len() < limit && !chunks.is_empty() {
                let chunk = chunks.remove(0);
                in_flight.push(self.download_chunk(remote_path, local_path, chunk));
            }
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }

    async fn upload_chunks(
        &self,
        remote_target: &str,
        local: &str,
        offset: u64,
        size: u64,
        max_workers: usize,
    ) -> Result<(), String> {
        let mut chunks = build_chunks(offset, size);
        let mut in_flight = FuturesUnordered::new();
        let limit = max_workers.max(1);

        while !chunks.is_empty() || !in_flight.is_empty() {
            while in_flight.len() < limit && !chunks.is_empty() {
                let chunk = chunks.remove(0);
                in_flight.push(self.upload_chunk(remote_target, local, chunk));
            }
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }

    async fn download_chunk(&self, remote_path: &str, local_path: &str, chunk: super::types::Chunk) -> Result<(), String> {
        let mut remote_file = self
            .sftp
            .open(remote_path.to_string())
            .await
            .map_err(|err| err.to_string())?;
        remote_file
            .seek(std::io::SeekFrom::Start(chunk.offset))
            .await
            .map_err(|err| err.to_string())?;
        let mut local_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(local_path)
            .await
            .map_err(|err| err.to_string())?;
        local_file
            .seek(std::io::SeekFrom::Start(chunk.offset))
            .await
            .map_err(|err| err.to_string())?;

        let mut remaining = chunk.len;
        let mut buf = vec![0u8; 32 * 1024];
        while remaining > 0 {
            let want = remaining.min(buf.len() as u64) as usize;
            let n = remote_file.read(&mut buf[..want]).await.map_err(|err| err.to_string())?;
            if n == 0 {
                return Err("early eof".to_string());
            }
            local_file
                .write_all(&buf[..n])
                .await
                .map_err(|err| err.to_string())?;
            remaining -= n as u64;
        }
        local_file.flush().await.map_err(|err| err.to_string())
    }

    async fn upload_chunk(&self, remote_target: &str, local: &str, chunk: super::types::Chunk) -> Result<(), String> {
        let mut local_file = fs::File::open(local).await.map_err(|err| err.to_string())?;
        local_file
            .seek(std::io::SeekFrom::Start(chunk.offset))
            .await
            .map_err(|err| err.to_string())?;
        let mut remote_file = self
            .sftp
            .open_with_flags(
                remote_target.to_string(),
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::READ,
            )
            .await
            .map_err(|err| err.to_string())?;
        remote_file
            .seek(std::io::SeekFrom::Start(chunk.offset))
            .await
            .map_err(|err| err.to_string())?;

        let mut remaining = chunk.len;
        let mut buf = vec![0u8; 32 * 1024];
        while remaining > 0 {
            let want = remaining.min(buf.len() as u64) as usize;
            let n = local_file.read(&mut buf[..want]).await.map_err(|err| err.to_string())?;
            if n == 0 {
                return Err("early eof".to_string());
            }
            remote_file
                .write_all(&buf[..n])
                .await
                .map_err(|err| err.to_string())?;
            remaining -= n as u64;
        }
        remote_file.flush().await.map_err(|err| err.to_string())
    }

    async fn collect_remote_jobs(
        &self,
        root_remote: &str,
        local_root: &Path,
        current_remote: &str,
        jobs: &mut Vec<DownloadJob>,
    ) -> Result<(), String> {
        for entry in self.sftp.read_dir(current_remote.to_string()).await.map_err(|err| err.to_string())? {
            let remote_path = remote_join(current_remote, &entry.file_name());
            let rel = remote_path
                .strip_prefix(root_remote)
                .unwrap_or(&remote_path)
                .trim_start_matches('/');
            let local_path = local_root.join(rel);
            if entry.metadata().is_dir() {
                fs::create_dir_all(&local_path).await.map_err(|err| err.to_string())?;
                Box::pin(self.collect_remote_jobs(root_remote, local_root, &remote_path, jobs)).await?;
            } else {
                jobs.push(DownloadJob {
                    remote: remote_path,
                    local: local_path.display().to_string(),
                });
            }
        }
        Ok(())
    }

    async fn collect_local_jobs(&self, current_local: &Path, current_remote: &str, jobs: &mut Vec<UploadJob>) -> Result<(), String> {
        let mut entries = fs::read_dir(current_local).await.map_err(|err| err.to_string())?;
        while let Some(entry) = entries.next_entry().await.map_err(|err| err.to_string())? {
            let local_path = entry.path();
            let remote_path = remote_join(current_remote, &entry.file_name().to_string_lossy());
            let meta = entry.metadata().await.map_err(|err| err.to_string())?;
            if meta.is_dir() {
                ensure_remote_dir(&self.sftp, &remote_path).await?;
                Box::pin(self.collect_local_jobs(&local_path, &remote_path, jobs)).await?;
            } else {
                jobs.push(UploadJob {
                    local: local_path.display().to_string(),
                    remote: remote_path,
                });
            }
        }
        Ok(())
    }

    async fn run_download_jobs(&self, jobs: Vec<DownloadJob>, options: TransferOptions) -> Result<(), String> {
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
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }

    async fn run_upload_jobs(&self, jobs: Vec<UploadJob>, options: TransferOptions) -> Result<(), String> {
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
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }
}
