use failure::{Error, Fail};
use std::io::ErrorKind;
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
