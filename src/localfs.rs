use remotefs::fs::{File, Metadata, ReadStream, UnixPex, Welcome, WriteStream};
use remotefs::{RemoteError, RemoteErrorType, RemoteFs, RemoteResult};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct LocalFs {
    pub pwd: PathBuf,
}

impl LocalFs {
    pub fn new(start: PathBuf) -> Self {
        Self { pwd: start }
    }
}

impl RemoteFs for LocalFs {
    fn connect(&mut self) -> RemoteResult<Welcome> {
        Ok(Welcome::default())
    }

    fn disconnect(&mut self) -> RemoteResult<()> {
        Ok(())
    }

    fn is_connected(&mut self) -> bool {
        true
    }

    fn pwd(&mut self) -> RemoteResult<PathBuf> {
        Ok(self.pwd.clone())
    }

    fn change_dir(&mut self, dir: &Path) -> RemoteResult<PathBuf> {
        let dir = self.pwd.join(dir);
        //trace!("changing directory to {}", ,dir.display());
        // check if directory exists
        if self.stat(dir.as_path())?.is_dir() {
            self.pwd = dir;
            //debug!("new working directory: {}", self.pwd.display());
            Ok(self.pwd.clone())
        } else {
            //error!("cannot enter directory {}. Not a directory", dir.display());
            Err(RemoteError::new_ex(
                RemoteErrorType::BadFile,
                "not a directory",
            ))
        }
    }

    fn list_dir(&mut self, path: &Path) -> RemoteResult<Vec<File>> {
        let dir = self.pwd.join(path);
        //trace!("listing files at {}", path);

        let dirents = std::fs::read_dir(dir)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::CouldNotOpenFile, e))?;

        Ok(dirents
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|d| {
                if let Ok(ft) = d.file_type() {
                    if ft.is_dir() || ft.is_file() {
                        Some(self.stat(&d.path()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .flatten()
            .collect())
    }

    fn stat(&mut self, path: &Path) -> RemoteResult<File> {
        let path = self.pwd.join(path);
        //trace!("get stat for {}", path);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::StatFailed, e))?;

        let file_type = if metadata.file_type().is_dir() {
            remotefs::fs::FileType::Directory
        } else if metadata.file_type().is_file() {
            remotefs::fs::FileType::File
        } else {
            remotefs::fs::FileType::Symlink
        };

        // TODO: Support Unix Permissions
        let rfs_mt = remotefs::fs::Metadata {
            accessed: metadata.accessed().ok(),
            created: metadata.created().ok(),
            gid: None,
            mode: None,
            modified: metadata.modified().ok(),
            size: metadata.len(),
            symlink: None,
            file_type,
            uid: None,
        };

        Ok(remotefs::fs::File {
            path,
            metadata: rfs_mt,
        })
    }

    fn setstat(&mut self, _path: &Path, _metadata: Metadata) -> RemoteResult<()> {
        // TODO: Support Unix Permissions
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn exists(&mut self, path: &Path) -> RemoteResult<bool> {
        //trace!("checking if {} exists...", path.display());
        match self.stat(path) {
            Ok(_) => Ok(true),
            Err(RemoteError {
                kind: RemoteErrorType::StatFailed,
                ..
            }) => Ok(false),
            Err(err) => Err(err),
        }
    }

    fn remove_file(&mut self, path: &Path) -> RemoteResult<()> {
        let path = self.pwd.join(path);
        //trace!("removing file {}", path);
        std::fs::remove_file(path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::CouldNotRemoveFile, e))
    }

    fn remove_dir(&mut self, path: &Path) -> RemoteResult<()> {
        let path = self.pwd.join(path);
        //trace!("removing directory at {}", path);
        std::fs::remove_dir(path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::CouldNotRemoveFile, e))
    }

    fn create_dir(&mut self, path: &Path, _mode: UnixPex) -> RemoteResult<()> {
        if self.exists(path)? {
            return Err(RemoteError::new(RemoteErrorType::DirectoryAlreadyExists));
        }
        let path = self.pwd.join(path);
        //trace!("making directory at {}", path);
        // check if directory exists
        std::fs::create_dir(path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::FileCreateDenied, e))
    }

    fn symlink(&mut self, _path: &Path, _target: &Path) -> RemoteResult<()> {
        // TODO: Depending of the platform
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn copy(&mut self, src: &Path, dest: &Path) -> RemoteResult<()> {
        let src = self.pwd.join(src);
        let dest = self.pwd.join(dest);
        std::fs::copy(src, dest)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::ProtocolError, e))?;
        Ok(())
    }

    fn mov(&mut self, src: &Path, dest: &Path) -> RemoteResult<()> {
        let src = self.pwd.join(src);
        let dest = self.pwd.join(dest);
        // trace!("moving {} to {}", src, dest);
        // check if directory exists
        std::fs::rename(src, dest)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::ProtocolError, e))
    }

    fn exec(&mut self, _cmd: &str) -> RemoteResult<(u32, String)> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn append_file(
        &mut self,
        path: &Path,
        _metadata: &Metadata,
        mut reader: Box<dyn Read>,
    ) -> RemoteResult<u64> {
        let path = self.pwd.join(path);
        //trace!("opening file at {} for append", path);
        let mut ops = std::fs::OpenOptions::new();
        ops.create(true).write(true).append(true);

        let mut file = ops
            .open(&path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::FileCreateDenied, e))?;
        std::io::copy(&mut reader, &mut file)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::IoError, e))
    }

    fn create_file(
        &mut self,
        path: &Path,
        _metadata: &Metadata,
        mut reader: Box<dyn Read>,
    ) -> RemoteResult<u64> {
        let path = self.pwd.join(path);
        //trace!("opening file at {} for create", path);
        let mut ops = std::fs::OpenOptions::new();
        ops.create(true).write(true).append(false).truncate(true);

        let mut file = ops
            .open(&path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::FileCreateDenied, e))?;
        std::io::copy(&mut reader, &mut file)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::IoError, e))
    }

    fn open_file(&mut self, path: &Path, mut dest: Box<dyn Write + Send>) -> RemoteResult<u64> {
        let path = self.pwd.join(path);
        //trace!("opening file at {} for open", path);
        let mut ops = std::fs::OpenOptions::new();
        ops.read(true);

        let mut file = ops
            .open(&path)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::CouldNotOpenFile, e))?;
        std::io::copy(&mut file, &mut dest)
            .map_err(|e| RemoteError::new_ex(RemoteErrorType::IoError, e))
    }

    fn append(&mut self, _path: &Path, _metadata: &Metadata) -> RemoteResult<WriteStream> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn create(&mut self, _path: &Path, _metadata: &Metadata) -> RemoteResult<WriteStream> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn open(&mut self, _path: &Path) -> RemoteResult<ReadStream> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }
}
