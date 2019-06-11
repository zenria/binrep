use failure::{Error, Fail};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[derive(Fail, Debug)]
#[fail(display = "{} is not a directory", 0)]
pub struct PathIsNotADirectoryError(pub String);

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

pub fn read_sane_from_file<P: AsRef<Path>, S: DeserializeOwned>(file: P) -> Result<S, Error> {
    let mut file = File::open(&file)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    // Parse config file
    Ok(sane::from_str(&s)?)
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
