use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::ssh::{ClientOptions, Session, LOG};

use super::paths::{
    build_chunks, canonicalize_or, ensure_remote_dir, remote_join, remote_parent, resolve_local_target,
    resolve_remote_target,
};
use super::types::{Client, DownloadJob, ProgressReporter, TransferOptions, UploadJob};

impl Client {
    pub async fn connect(options: ClientOptions) -> Result<Self, String> {
        let mut session = Session::connect(options).await?;
        LOG.log(format_args!("ssh client ready"));
        let sftp = match session.open_sftp().await {
            Ok(sftp) => sftp,
            Err(err) => {
                LOG.log(format_args!("cannot create SFTP client: {}", err));
                return Err(err);
            }
        };
        LOG.log(format_args!("SFTP client created"));
        Ok(Self { session, sftp })
    }

    pub async fn close(&mut self) -> Result<(), String> {
        let _ = self.sftp.close().await;
        LOG.log(format_args!("SFTP connection lost"));
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
        self.get_file_with_options_and_progress(remote, local, options, None)
            .await
    }

    pub async fn get_file_with_options_and_progress(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
        progress: Option<Arc<dyn ProgressReporter>>,
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
            LOG.log(format_args!("File already fully downloaded."));
            return Ok(());
        }

        // Small or effectively complete files stay on the resumable single-stream path; larger
        // transfers switch to ranged workers so they behave like the Go worker pool.
        let (progress_tx, progress_join) =
            self.start_progress(progress, size, offset, remote_path.clone(), options.max_workers.max(1));
        let result = if options.max_workers <= 1 || size.saturating_sub(offset) as usize <= super::DEFAULT_CHUNK_SIZE {
            self.copy_file_download(&remote_path, &local_path, offset, progress_tx.clone())
                .await
        } else {
            self.download_chunks(
                &remote_path,
                &local_path,
                offset,
                size,
                options.max_workers,
                progress_tx.clone(),
            )
            .await
        };
        drop(progress_tx);
        self.finish_progress(progress_join).await?;
        result?;
        set_local_permissions(Path::new(&local_path), remote_meta.permissions).await
    }

    pub async fn put_file_with_options(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
    ) -> Result<(), String> {
        self.put_file_with_options_and_progress(remote, local, options, None)
            .await
    }

    pub async fn put_file_with_options_and_progress(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Result<(), String> {
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if local_meta.is_dir() {
            return Err(format!("local path is not a file: {local}"));
        }

        let remote_target = resolve_remote_target(remote, local, &self.sftp).await?;
        LOG.log(format_args!("remotePath {}", remote_target));
        let offset = match self.sftp.metadata(remote_target.clone()).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        let size = local_meta.len();
        if offset >= size {
            LOG.log(format_args!("File already fully uploaded."));
            return Ok(());
        }

        if let Some(parent) = remote_parent(&remote_target) {
            ensure_remote_dir(&self.sftp, &parent).await?;
        }

        let (progress_tx, progress_join) =
            self.start_progress(progress, size, offset, remote_target.clone(), options.max_workers.max(1));
        LOG.log(format_args!("Using {} workers", options.max_workers));
        // Uploads use the same split: resume with a single stream for small tails, or fan out into
        // independent ranged writers when parallel workers are actually beneficial.
        let result = if options.max_workers <= 1 || size.saturating_sub(offset) as usize <= super::DEFAULT_CHUNK_SIZE {
            self.copy_file_upload(&remote_target, local, offset, progress_tx.clone())
                .await
        } else {
            self.upload_chunks(
                &remote_target,
                local,
                offset,
                size,
                options.max_workers,
                progress_tx.clone(),
            )
            .await
        };
        drop(progress_tx);
        self.finish_progress(progress_join).await?;
        result?;
        set_remote_permissions(&self.sftp, &remote_target, local_mode(&local_meta)).await
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
        self.get_recursive_with_options_and_progress(remote, local, options, None)
            .await
    }

    pub async fn get_recursive_with_options_and_progress(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
        progress: Option<Arc<dyn ProgressReporter>>,
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

        let root_name = Path::new(&remote_root)
            .file_name()
            .ok_or_else(|| format!("invalid remote path: {remote_root}"))?
            .to_string_lossy()
            .into_owned();
        let local_root = Path::new(local).join(root_name);
        fs::create_dir_all(&local_root).await.map_err(|err| err.to_string())?;
        set_local_permissions(&local_root, remote_meta.permissions).await?;

        let mut jobs = Vec::new();
        self.collect_remote_jobs(&remote_root, &local_root, &remote_root, &mut jobs)
            .await?;
        self.run_download_jobs(jobs, options, progress).await
    }

    pub async fn put_recursive_with_options(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
    ) -> Result<(), String> {
        self.put_recursive_with_options_and_progress(remote, local, options, None)
            .await
    }

    pub async fn put_recursive_with_options_and_progress(
        &self,
        remote: &str,
        local: &str,
        options: TransferOptions,
        progress: Option<Arc<dyn ProgressReporter>>,
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
        self.run_upload_jobs(jobs, options, progress).await
    }

    async fn copy_file_download(
        &self,
        remote_path: &str,
        local_path: &str,
        offset: u64,
        progress_tx: Option<mpsc::Sender<u64>>,
    ) -> Result<(), String> {
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

        copy_stream_with_progress(&mut remote_file, &mut local_file, progress_tx).await?;
        local_file.flush().await.map_err(|err| err.to_string())?;
        remote_file.shutdown().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn copy_file_upload(
        &self,
        remote_target: &str,
        local: &str,
        offset: u64,
        progress_tx: Option<mpsc::Sender<u64>>,
    ) -> Result<(), String> {
        let mut local_file = fs::File::open(local).await.map_err(|err| err.to_string())?;
        let mut remote_file = self
            .sftp
            .open_with_flags(remote_target.to_string(), OpenFlags::CREATE | OpenFlags::WRITE)
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

        copy_stream_with_progress(&mut local_file, &mut remote_file, progress_tx).await?;
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
        progress_tx: Option<mpsc::Sender<u64>>,
    ) -> Result<(), String> {
        let mut chunks = VecDeque::from(build_chunks(offset, size));
        let mut in_flight = FuturesUnordered::new();
        let limit = max_workers.max(1);

        while !chunks.is_empty() || !in_flight.is_empty() {
            // Keep the worker set full, but never exceed the configured concurrency for a single
            // file. Each worker owns a fixed byte range.
            while in_flight.len() < limit && !chunks.is_empty() {
                let chunk = chunks.pop_front().expect("chunk available");
                in_flight.push(self.retry_download_chunk(
                    remote_path.to_string(),
                    local_path.to_string(),
                    chunk,
                    progress_tx.clone(),
                ));
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
        progress_tx: Option<mpsc::Sender<u64>>,
    ) -> Result<(), String> {
        let mut chunks = VecDeque::from(build_chunks(offset, size));
        let mut in_flight = FuturesUnordered::new();
        let limit = max_workers.max(1);

        while !chunks.is_empty() || !in_flight.is_empty() {
            // Upload workers mirror the download side: each task retries its own range until it
            // is fully persisted, then reports the completed byte count once.
            while in_flight.len() < limit && !chunks.is_empty() {
                let chunk = chunks.pop_front().expect("chunk available");
                in_flight.push(self.retry_upload_chunk(
                    remote_target.to_string(),
                    local.to_string(),
                    chunk,
                    progress_tx.clone(),
                ));
            }
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }

    async fn retry_download_chunk(
        &self,
        remote_path: String,
        local_path: String,
        chunk: super::types::Chunk,
        progress_tx: Option<mpsc::Sender<u64>>,
    ) -> Result<(), String> {
        loop {
            match self.download_chunk(&remote_path, &local_path, chunk.clone()).await {
                Ok(written) => {
                    if let Some(tx) = &progress_tx {
                        let _ = tx.send(written).await;
                    }
                    return Ok(());
                }
                Err(_) => continue,
            }
        }
    }

    async fn retry_upload_chunk(
        &self,
        remote_target: String,
        local: String,
        chunk: super::types::Chunk,
        progress_tx: Option<mpsc::Sender<u64>>,
    ) -> Result<(), String> {
        loop {
            match self.upload_chunk(&remote_target, &local, chunk.clone()).await {
                Ok(written) => {
                    if let Some(tx) = &progress_tx {
                        let _ = tx.send(written).await;
                    }
                    return Ok(());
                }
                Err(_) => continue,
            }
        }
    }

    async fn download_chunk(
        &self,
        remote_path: &str,
        local_path: &str,
        chunk: super::types::Chunk,
    ) -> Result<u64, String> {
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
        local_file.flush().await.map_err(|err| err.to_string())?;
        Ok(chunk.len)
    }

    async fn upload_chunk(
        &self,
        remote_target: &str,
        local: &str,
        chunk: super::types::Chunk,
    ) -> Result<u64, String> {
        let mut local_file = fs::File::open(local).await.map_err(|err| err.to_string())?;
        local_file
            .seek(std::io::SeekFrom::Start(chunk.offset))
            .await
            .map_err(|err| err.to_string())?;
        let mut remote_file = self
            .sftp
            .open_with_flags(remote_target.to_string(), OpenFlags::CREATE | OpenFlags::WRITE)
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
        remote_file.flush().await.map_err(|err| err.to_string())?;
        Ok(chunk.len)
    }

    async fn collect_remote_jobs(
        &self,
        root_remote: &str,
        local_root: &Path,
        current_remote: &str,
        jobs: &mut Vec<DownloadJob>,
    ) -> Result<(), String> {
        for entry in self
            .sftp
            .read_dir(current_remote.to_string())
            .await
            .map_err(|err| err.to_string())?
        {
            let remote_path = remote_join(current_remote, &entry.file_name());
            let rel = remote_path
                .strip_prefix(root_remote)
                .unwrap_or(&remote_path)
                .trim_start_matches('/');
            let local_path = local_root.join(rel);
            if entry.metadata().is_dir() {
                fs::create_dir_all(&local_path).await.map_err(|err| err.to_string())?;
                set_local_permissions(&local_path, entry.metadata().permissions).await?;
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

    async fn collect_local_jobs(
        &self,
        current_local: &Path,
        current_remote: &str,
        jobs: &mut Vec<UploadJob>,
    ) -> Result<(), String> {
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

    async fn run_download_jobs(
        &self,
        jobs: Vec<DownloadJob>,
        options: TransferOptions,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Result<(), String> {
        let mut iter = jobs.into_iter();
        let mut in_flight = FuturesUnordered::new();
        let limit = options.concurrent_files.max(1);

        while !in_flight.is_empty() || iter.len() > 0 {
            // Recursive transfers bound file-level concurrency separately from chunk-level
            // concurrency so one large file cannot starve the rest of the tree.
            while in_flight.len() < limit {
                let Some(job) = iter.next() else {
                    break;
                };
                let remote = job.remote;
                let local = job.local;
                let progress = progress.clone();
                in_flight.push(async move {
                    self.get_file_with_options_and_progress(
                        &remote,
                        &local,
                        TransferOptions::new(options.max_workers, 1),
                        progress,
                    )
                    .await
                });
            }
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }

    async fn run_upload_jobs(
        &self,
        jobs: Vec<UploadJob>,
        options: TransferOptions,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Result<(), String> {
        let mut iter = jobs.into_iter();
        let mut in_flight = FuturesUnordered::new();
        let limit = options.concurrent_files.max(1);

        while !in_flight.is_empty() || iter.len() > 0 {
            // Each recursive upload job reuses the single-file machinery, but the outer queue caps
            // how many distinct files are transferred at once.
            while in_flight.len() < limit {
                let Some(job) = iter.next() else {
                    break;
                };
                let remote = job.remote;
                let local = job.local;
                let progress = progress.clone();
                in_flight.push(async move {
                    self.put_file_with_options_and_progress(
                        &remote,
                        &local,
                        TransferOptions::new(options.max_workers, 1),
                        progress,
                    )
                    .await
                });
            }
            if let Some(result) = in_flight.next().await {
                result?;
            }
        }
        Ok(())
    }

    fn start_progress(
        &self,
        progress: Option<Arc<dyn ProgressReporter>>,
        size: u64,
        offset: u64,
        file_name: String,
        capacity: usize,
    ) -> (Option<mpsc::Sender<u64>>, Option<tokio::task::JoinHandle<()>>) {
        let Some(progress) = progress else {
            return (None, None);
        };
        let (progress_tx, progress_rx) = mpsc::channel(capacity.max(1));
        let progress_join = progress.spawn(size, offset, file_name, progress_rx);
        (Some(progress_tx), Some(progress_join))
    }

    async fn finish_progress(
        &self,
        progress_join: Option<tokio::task::JoinHandle<()>>,
    ) -> Result<(), String> {
        if let Some(join) = progress_join {
            join.await.map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

async fn copy_stream_with_progress<R, W>(
    reader: &mut R,
    writer: &mut W,
    progress_tx: Option<mpsc::Sender<u64>>,
) -> Result<(), String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 32 * 1024];
    loop {
        let n = reader.read(&mut buf).await.map_err(|err| err.to_string())?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .await
            .map_err(|err| err.to_string())?;
        if let Some(tx) = &progress_tx {
            let _ = tx.send(n as u64).await;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn local_mode(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode()
}

#[cfg(not(unix))]
fn local_mode(metadata: &std::fs::Metadata) -> u32 {
    if metadata.permissions().readonly() {
        0o555
    } else {
        0o777
    }
}

async fn set_remote_permissions(
    sftp: &russh_sftp::client::SftpSession,
    remote_path: &str,
    permissions: u32,
) -> Result<(), String> {
    let mut metadata = FileAttributes::empty();
    metadata.permissions = Some(permissions);
    sftp.set_metadata(remote_path.to_string(), metadata)
        .await
        .map_err(|err| err.to_string())
}

async fn set_local_permissions(path: &Path, permissions: Option<u32>) -> Result<(), String> {
    let Some(permissions) = permissions else {
        return Ok(());
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, std::fs::Permissions::from_mode(permissions & 0o777))
            .await
            .map_err(|err| err.to_string())?;
    }
    #[cfg(not(unix))]
    {
        let readonly = permissions & 0o222 == 0;
        let mut perms = fs::metadata(path).await.map_err(|err| err.to_string())?.permissions();
        perms.set_readonly(readonly);
        fs::set_permissions(path, perms).await.map_err(|err| err.to_string())?;
    }
    Ok(())
}
