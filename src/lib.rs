#![allow(dead_code)]
#![allow(unused_variables)]

use crate::backend::Backend;
use crate::metadata::{Artifacts, Versions};
use failure::{Error, Fail};
use std::io;
use std::io::{Read, Write};
use std::path::Path;

pub mod backend;
pub mod config;
pub mod metadata;

pub type Version = String;

pub struct Repository {
    backend: Box<Backend>,
}

#[derive(Debug, Fail)]
#[fail(display = "Wrong artifact naming, only alphanumeric characters and -_ are allowed!")]
struct ArtifactNameError;

fn validate_artifact_name(name: &str) -> Result<(), ArtifactNameError> {
    if name.len() == 0 {
        return Err(ArtifactNameError);
    }
    name.as_bytes().iter().try_for_each(|c| {
        if c.is_ascii_alphanumeric() || *c == '-' as u8 || *c == '_' as u8 {
            Ok(())
        } else {
            Err(ArtifactNameError)
        }
    })
}

impl Repository {
    pub fn list_artifacts(&self) -> Result<Vec<String>, Error> {
        Ok(sane::from_str::<Artifacts>(&self.backend.read_file("/actifacts.sane")?)?.artifacts)
    }

    pub fn list_artifact_versions(&self, artifact_name: &str) -> Result<Vec<Version>, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = vec![artifact_name, "/versions.sane"].into_iter().collect();
        Ok(sane::from_str::<Versions>(&self.backend.read_file(&path)?)?.versions)
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn validate_artifact_name() {
        super::validate_artifact_name("foo").unwrap();
        super::validate_artifact_name("-f_54321Affesoo").unwrap();
        assert!(super::validate_artifact_name(" ").is_err());
        assert!(super::validate_artifact_name("").is_err());
        assert!(super::validate_artifact_name("someé").is_err());
    }
}
