use crate::progress::ProgressReporter;
use std::error::Error;
use std::path::PathBuf;

pub mod file_backend;
pub mod s3_backend;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("resource not found")]
    ResourceNotFound,
    #[error("backend returned error: {cause}")]
    Other { cause: anyhow::Error },
}

impl From<anyhow::Error> for BackendError {
    fn from(e: anyhow::Error) -> Self {
        BackendError::Other { cause: e }
    }
}

pub trait Backend<T: ProgressReporter> {
    /// read a text file from specified path
    ///
    /// The path is relative to the ROOT of the backend
    fn read_file(&mut self, path: &str) -> Result<String, BackendError>;

    /// create text a file in the specified path
    ///
    /// The path is relative to the ROOT of the backend
    fn create_file(&mut self, path: &str, data: String) -> Result<(), BackendError>;

    fn push_file(&mut self, local: PathBuf, remote: &str) -> Result<(), BackendError>;

    /// Pull a file from the backend to a local file.
    ///
    /// It does not check if the local file exists!
    fn pull_file(&mut self, remote: &str, local: PathBuf) -> Result<(), BackendError>;
}
