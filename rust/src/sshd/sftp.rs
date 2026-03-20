use std::collections::HashMap;
use std::path::{Path, PathBuf};

use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

#[derive(Default)]
pub(super) struct SftpServer {
    next_handle: usize,
    handles: HashMap<String, SftpHandle>,
}

enum SftpHandle {
    File(fs::File),
    Dir {
        entries: Vec<File>,
        index: usize,
    },
}

impl russh_sftp::server::Handler for SftpServer {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = sftp_path(&filename);
        let file = fs::OpenOptions::from(std::fs::OpenOptions::from(pflags))
            .open(path)
            .await
            .map_err(map_io_error)?;
        let handle = self.allocate_handle(SftpHandle::File(file));
        Ok(Handle { id, handle })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(ok_status(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(map_io_error)?;
        let mut buf = vec![0u8; len as usize];
        let n = file.read(&mut buf).await.map_err(map_io_error)?;
        if n == 0 {
            return Err(StatusCode::Eof);
        }
        buf.truncate(n);
        Ok(Data { id, data: buf })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(map_io_error)?;
        file.write_all(&data).await.map_err(map_io_error)?;
        file.flush().await.map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let metadata = fs::symlink_metadata(sftp_path(&path))
            .await
            .map_err(map_io_error)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        let metadata = file.metadata().await.map_err(map_io_error)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        apply_attrs(Path::new(&sftp_path(&path)), &attrs).await?;
        Ok(ok_status(id))
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        handle: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        if let Some(size) = attrs.size {
            file.set_len(size).await.map_err(map_io_error)?;
        }
        Ok(ok_status(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let pathbuf = sftp_path(&path);
        let mut entries = fs::read_dir(&pathbuf).await.map_err(map_io_error)?;
        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
            let metadata = entry.metadata().await.map_err(map_io_error)?;
            files.push(File::new(
                entry.file_name().to_string_lossy().into_owned(),
                FileAttributes::from(&metadata),
            ));
        }
        let handle = self.allocate_handle(SftpHandle::Dir { entries: files, index: 0 });
        Ok(Handle { id, handle })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        let Some(SftpHandle::Dir { entries, index }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        if *index >= entries.len() {
            return Err(StatusCode::Eof);
        }
        let batch = entries[*index..].to_vec();
        *index = entries.len();
        Ok(Name { id, files: batch })
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        fs::remove_file(sftp_path(&filename))
            .await
            .map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        fs::create_dir_all(sftp_path(&path))
            .await
            .map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        fs::remove_dir(sftp_path(&path)).await.map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let normalized = if path.is_empty() || path == "." {
            ".".to_string()
        } else {
            path
        };
        let absolute = fs::canonicalize(sftp_path(&normalized))
            .await
            .unwrap_or_else(|_| PathBuf::from(normalized));
        Ok(Name {
            id,
            files: vec![File::dummy(absolute.to_string_lossy().into_owned())],
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let metadata = fs::metadata(sftp_path(&path)).await.map_err(map_io_error)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        fs::rename(sftp_path(&oldpath), sftp_path(&newpath))
            .await
            .map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let target = fs::read_link(sftp_path(&path)).await.map_err(map_io_error)?;
        Ok(Name {
            id,
            files: vec![File::dummy(target.to_string_lossy().into_owned())],
        })
    }

    async fn symlink(
        &mut self,
        _id: u32,
        linkpath: String,
        targetpath: String,
    ) -> Result<Status, Self::Error> {
        #[cfg(unix)]
        {
            tokio::fs::symlink(targetpath, sftp_path(&linkpath))
                .await
                .map_err(map_io_error)?;
            Ok(ok_status(id))
        }
        #[cfg(not(unix))]
        {
            let _ = (linkpath, targetpath);
            Err(StatusCode::OpUnsupported)
        }
    }
}

impl SftpServer {
    fn allocate_handle(&mut self, entry: SftpHandle) -> String {
        self.next_handle += 1;
        let handle = self.next_handle.to_string();
        self.handles.insert(handle.clone(), entry);
        handle
    }
}

fn sftp_path(path: &str) -> PathBuf {
    if path.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(path)
    }
}

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

fn map_io_error(err: std::io::Error) -> StatusCode {
    use std::io::ErrorKind;

    match err.kind() {
        ErrorKind::NotFound => StatusCode::NoSuchFile,
        ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        ErrorKind::AlreadyExists => StatusCode::Failure,
        ErrorKind::UnexpectedEof => StatusCode::Eof,
        _ => StatusCode::Failure,
    }
}

async fn apply_attrs(path: &Path, attrs: &FileAttributes) -> Result<(), StatusCode> {
    if let Some(size) = attrs.size {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .await
            .map_err(map_io_error)?;
        file.set_len(size).await.map_err(map_io_error)?;
    }
    #[cfg(unix)]
    if let Some(mode) = attrs.permissions {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o777))
            .await
            .map_err(map_io_error)?;
    }
    Ok(())
}
