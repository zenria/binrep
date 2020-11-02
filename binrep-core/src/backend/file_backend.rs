use crate::backend::{Backend, BackendError, ProgressReporter};
use crate::file_utils;
use anyhow::Error;
use std::fs::File;
use std::io::Write;
use std::io::{ErrorKind, Read};
use std::marker::PhantomData;
use std::path::PathBuf;

pub struct FileBackend<T: ProgressReporter> {
    root: PathBuf,
    _progress_reporter: PhantomData<T>,
}

impl<T: ProgressReporter> FileBackend<T> {
    pub fn new(root: &str) -> Self {
        FileBackend {
            root: PathBuf::from(root),
            _progress_reporter: PhantomData,
        }
    }

    fn mkdirs(&self, file_path: &PathBuf) -> Result<(), Error> {
        // check dir existence, create if is does not exists, throw an error
        // if the dir is not a dir ;)
        if let Some(dir) = file_path.parent() {
            file_utils::mkdirs(dir)?;
        } else {
            // No parent what is root ????
            Err(file_utils::PathIsNotADirectoryError(
                self.root.to_string_lossy().to_string(),
            ))?
        }
        Ok(())
    }
}

impl From<std::io::Error> for BackendError {
    fn from(ioe: std::io::Error) -> Self {
        match &ioe.kind() {
            ErrorKind::NotFound => BackendError::ResourceNotFound,
            _ => BackendError::Other { cause: ioe.into() },
        }
    }
}

impl<T: ProgressReporter> Backend<T> for FileBackend<T> {
    fn read_file(&mut self, path: &str) -> Result<String, BackendError> {
        let file_path = get_path(self.root.clone(), path);
        let mut ret = String::new();
        File::open(file_path)?.read_to_string(&mut ret)?;
        Ok(ret)
    }

    fn create_file(&mut self, path: &str, data: String) -> Result<(), BackendError> {
        let file_path = get_path(self.root.clone(), path);
        self.mkdirs(&file_path)?;
        let mut file = File::create(file_path)?;
        file.write_all(data.as_bytes())?;
        Ok(())
    }

    fn push_file(&mut self, local: PathBuf, remote: &str) -> Result<(), BackendError> {
        let remote_file_path = get_path(self.root.clone(), remote);
        self.mkdirs(&remote_file_path)?;
        std::fs::copy(local, remote_file_path)?;
        Ok(())
    }

    fn pull_file(&mut self, remote: &str, local: PathBuf) -> Result<(), BackendError> {
        let remote_file_path = get_path(self.root.clone(), remote);
        std::fs::copy(remote_file_path, local)?;
        Ok(())
    }
}

fn get_path(root: PathBuf, path: &str) -> PathBuf {
    path.split("/")
        .filter(|element| element.len() > 0)
        .fold(root, |mut path, path_element| {
            path.push(path_element);
            path
        })
}

#[cfg(test)]
mod test {
    use crate::backend::file_backend::FileBackend;
    use crate::backend::Backend;
    use crate::progress::NOOPProgress;
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn test_get_path() {
        assert_eq!(
            PathBuf::from("/var/lib/some/file.txt"),
            super::get_path(PathBuf::from("/var/lib"), "some/file.txt")
        );
        assert_eq!(
            PathBuf::from("/var/lib/some/file.txt"),
            super::get_path(PathBuf::from("/var/lib"), "some//file.txt")
        );
        assert_eq!(
            PathBuf::from("/var/lib/some/file.txt"),
            super::get_path(PathBuf::from("/var/lib/"), "some//file.txt")
        );
        assert_eq!(
            PathBuf::from("/var/lib/some/file.txt"),
            super::get_path(PathBuf::from("/var/lib"), "./some/file.txt")
        );
    }

    #[test]
    #[allow(unused_must_use)]
    fn test_backend() {
        let root = tempdir().unwrap();
        let mut bck: FileBackend<NOOPProgress> =
            super::FileBackend::new(&root.into_path().to_string_lossy());
        let data = "This is some data";
        bck.create_file("foo/bar/some.txt", data.to_string())
            .unwrap();
        bck.create_file("root.txt", data.to_string()).unwrap();
        assert_eq!(data, bck.read_file("foo/bar/some.txt").unwrap());
        assert_eq!(data, bck.read_file("/foo/bar/some.txt").unwrap()); // also works with starting slash ;)
        assert_eq!(data, bck.read_file("root.txt").unwrap());
        assert_eq!(data, bck.read_file("/root.txt").unwrap()); // also works with starting slash ;)

        bck.push_file("./Cargo.toml".into(), "Cargo.toml").unwrap();
        assert_file_equals("./Cargo.toml", bck.read_file("Cargo.toml").unwrap());

        bck.push_file("./Cargo.toml".into(), "/foo2/bar/othername.toml")
            .unwrap();
        assert_file_equals(
            "./Cargo.toml",
            bck.read_file("/foo2/bar/othername.toml").unwrap(),
        );

        let dest_dir = tempdir().unwrap();
        let mut dest_file = PathBuf::from(dest_dir.path());
        dest_file.push("othername.toml");

        bck.pull_file("/foo2/bar/othername.toml", dest_file.clone())
            .unwrap();
        bck.pull_file("/foo2/bar/othername.toml", dest_file.clone())
            .unwrap();
    }

    fn assert_file_equals<A: AsRef<Path>>(file: A, data: String) {
        let mut from_fs = String::new();
        File::open(file)
            .unwrap()
            .read_to_string(&mut from_fs)
            .unwrap();
        assert_eq!(from_fs, data);
    }
}
