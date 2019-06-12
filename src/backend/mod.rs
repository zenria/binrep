use failure::{Error, Fail};
use std::path::PathBuf;

pub mod file_backend;
pub mod s3_backend;

#[derive(Debug, Fail)]
pub enum BackendError {
    #[fail(display = "resource not found")]
    ResourceNotFound,
    #[fail(display = "backend returned error: {}", cause)]
    Other { cause: Error },
}

impl From<Error> for BackendError {
    fn from(e: Error) -> Self {
        BackendError::Other { cause: e }
    }
}

pub trait Backend {
    /// read a text file from specified path
    ///
    /// The path is relative to the ROOT of the backend
    fn read_file(&self, path: &str) -> Result<String, BackendError>;

    /// create text a file in the specified path
    ///
    /// The path is relative to the ROOT of the backend
    fn create_file(&self, path: &str, data: String) -> Result<(), BackendError>;

    fn push_file(&self, local: PathBuf, remote: &str) -> Result<(), BackendError>;

    /// Pull a file from the backend to a local file.
    ///
    /// It does not check if the local file exists!
    fn pull_file(&self, remote: &str, local: PathBuf) -> Result<(), BackendError>;
}
