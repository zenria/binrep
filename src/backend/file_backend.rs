use crate::backend::Backend;
use failure::Error;
use failure::Fail;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

pub struct FileBackend {
    root: PathBuf,
}

#[derive(Fail, Debug)]
pub enum FileBackendError {
    #[fail(display = "Not a directory: {} ", _0)]
    NotADirectory(String),
}

impl FileBackend {
    pub fn new(root: &str) -> Self {
        FileBackend {
            root: PathBuf::from(root),
        }
    }

    fn mkdirs(&self, file_path: &PathBuf) -> Result<(), Error> {
        // check dir existence, create if is does not exists, throw an error
        // if the dir is not a dir ;)
        if let Some(dir) = file_path.parent() {
            if let Ok(dir_metadata) = std::fs::metadata(dir) {
                if !dir_metadata.is_dir() {
                    Err(FileBackendError::NotADirectory(
                        dir.to_string_lossy().to_string(),
                    ))?
                }
            } else {
                std::fs::create_dir_all(dir)?
            }
        } else {
            // No parent what is root ????
            Err(FileBackendError::NotADirectory(
                self.root.to_string_lossy().to_string(),
            ))?
        }
        Ok(())
    }
}

impl Backend for FileBackend {
    fn read_file(&self, path: &str) -> Result<String, Error> {
        let file_path = get_path(self.root.clone(), path);
        let mut ret = String::new();
        File::open(file_path)?.read_to_string(&mut ret)?;
        Ok(ret)
    }

    fn create_file(&self, path: &str, data: String) -> Result<(), Error> {
        let file_path = get_path(self.root.clone(), path);
        self.mkdirs(&file_path)?;
        let mut file = File::create(file_path)?;
        file.write_all(data.as_bytes())?;
        Ok(())
    }

    fn push_file(&self, local: PathBuf, remote: &str) -> Result<(), Error> {
        let remote_file_path = get_path(self.root.clone(), remote);
        self.mkdirs(&remote_file_path)?;
        std::fs::copy(local, remote_file_path)?;
        Ok(())
    }

    fn pull_file(&self, remote: &str, local_directory: PathBuf) -> Result<(), Error> {
        let remote_file_path = get_path(self.root.clone(), remote);
        std::fs::copy(remote_file_path, local_directory)?;
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
    use crate::backend::Backend;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};

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
        std::fs::remove_dir_all("./tests-data");
        let rand_string: String = thread_rng().sample_iter(&Alphanumeric).take(30).collect();
        let mut root = String::from("./tests-data/");
        root.push_str(&rand_string);
        let bck = super::FileBackend::new(&root);
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
