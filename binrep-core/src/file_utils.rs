use anyhow::Error;
use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[derive(thiserror::Error, Debug)]
#[error("{0} is not a directory")]
pub struct PathIsNotADirectoryError(pub String);

pub struct LockFile<P: AsRef<Path>> {
    lock_file_path: P,
    lock_file: File,
}

impl<P: AsRef<Path>> LockFile<P> {
    pub fn create_and_lock(lock_file_path: P) -> Result<Self, Error> {
        let lock_file = File::create(&lock_file_path)?;
        lock_file.try_lock_exclusive()?;
        Ok(Self {
            lock_file,
            lock_file_path,
        })
    }
}

impl<P: AsRef<Path>> Drop for LockFile<P> {
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        self.lock_file.unlock();
        std::fs::remove_file(&self.lock_file_path);
    }
}

pub fn mkdirs<P: AsRef<Path>>(dir: P) -> Result<(), Error> {
    if let Err(e) = std::fs::create_dir_all(&dir) {
        match e.kind() {
            ErrorKind::AlreadyExists => {
                // dir or file exists
                // let check the path is really a directory
                let meta = std::fs::metadata(&dir)?;
                if !meta.is_dir() {
                    Err(PathIsNotADirectoryError(
                        dir.as_ref().to_string_lossy().into(),
                    ))?;
                }
            }
            _ => Err(e)?,
        }
    }
    Ok(())
}

pub fn mv<S: AsRef<Path>, D: AsRef<Path>>(src: S, dst: D) -> Result<(), std::io::Error> {
    info!(
        "mv {} to {}",
        src.as_ref().to_string_lossy(),
        dst.as_ref().to_string_lossy()
    );
    match std::fs::rename(src.as_ref(), dst.as_ref()) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            ErrorKind::Other => std::fs::copy(src, dst).map(|_| ()),
            _ => Err(e),
        },
    }
}

pub fn path_concat2<T: AsRef<Path>, U: AsRef<Path>>(p1: T, p2: U) -> PathBuf {
    [p1.as_ref(), p2.as_ref().into()]
        .iter()
        .collect::<PathBuf>()
}

pub fn read_sane_from_file<P: AsRef<Path>, D: DeserializeOwned>(file: P) -> Result<D, Error> {
    let mut file = File::open(&file)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    // Parse config file
    Ok(sane::from_str(&s)?)
}

pub fn write_sane_to_file<P: AsRef<Path>, S: Serialize>(file: P, meta: &S) -> Result<(), Error> {
    let mut file = File::create(file)?;
    file.write_all(sane::to_string(meta)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
fn test_mkdirs() {
    // mkdirs on existing file => error
    assert!(mkdirs("Cargo.toml").is_err());
    // mkdir on a already existing dir just does not fail
    mkdirs(tempdir().unwrap().path()).unwrap();
    mkdirs("./src").unwrap();

    let mut non_existing = PathBuf::from(tempdir().unwrap().path());
    non_existing.push("./cannot-exist");
    mkdirs(&non_existing).unwrap();
    assert!(std::fs::metadata(&non_existing).unwrap().is_dir());
}
