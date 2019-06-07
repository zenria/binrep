use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub mod file_backend;

pub trait Backend {
    /// read a text file from specified path
    ///
    /// The path is relative to the ROOT of the backend
    fn read_file(&self, path: &str) -> Result<String, failure::Error>;

    /// create text a file in the specified path
    ///
    /// The path is relative to the ROOT of the backend
    fn create_file(&self, path: &str, data: String) -> Result<(), failure::Error>;

    fn push_file(&self, local: PathBuf, remote: &str) -> Result<(), failure::Error>;

    fn pull_file(&self, remote: &str, local_directory: PathBuf) -> Result<(), failure::Error>;
}
