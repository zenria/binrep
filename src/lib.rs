// dev allows ;)
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

#[macro_use]
extern crate log;

use crate::backend::file_backend::FileBackend;
use crate::backend::Backend;
use crate::config::{BackendType, Config};
use crate::crypto::Signer;
use crate::metadata::{Artifact, Artifacts, ChecksumMethod, Latest, SignatureMethod, Versions};
use crate::path::artifacts;
use core::borrow::Borrow;
use failure::{Error, Fail};
use ring::digest::{Algorithm, Digest};
use semver::Version;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub mod backend;
pub mod config;
pub mod config_resolver;
mod crypto;
pub mod metadata;
mod path;

pub struct Repository {
    backend: Box<Backend>,
    config: Config,
}

#[derive(Debug, Fail)]
enum RepositoryError {
    #[fail(display = "Wrong artifact naming, only alphanumeric characters and -_. are allowed")]
    ArtifactNameError,
    #[fail(display = "Artifact version already exists")]
    ArtifactVersionAlreadyExists,
}

fn validate_artifact_name(name: &str) -> Result<(), RepositoryError> {
    if name.len() == 0 {
        return Err(RepositoryError::ArtifactNameError);
    }
    name.as_bytes().iter().try_for_each(|c| {
        if c.is_ascii_alphanumeric() || *c == '-' as u8 || *c == '_' as u8 || *c == '.' as u8 {
            Ok(())
        } else {
            Err(RepositoryError::ArtifactNameError)
        }
    })
}

impl Repository {
    pub fn new(config: Config) -> Self {
        // Construct the backend
        let backend = match &config.backend.backend_type {
            BackendType::File => Box::new(FileBackend::new(&config.backend.root)),
            BackendType::S3 => unimplemented!(),
        };
        Self { backend, config }
    }

    /// Initialize the repository, do nothing if the repository is already initialized.
    ///
    /// Always returns the Artifacts list
    fn init(&self) -> Result<Artifacts, Error> {
        match self.list_artifacts() {
            Ok(artifacts) => Ok(artifacts),
            Err(_) => {
                let new_artifacts = Artifacts::new();
                self.write_artifacts(&new_artifacts)?;
                Ok(new_artifacts)
            }
        }
    }

    fn write_artifacts(&self, artifacts: &Artifacts) -> Result<(), Error> {
        self.backend
            .create_file(path::artifacts(), sane::to_string(artifacts)?)
    }

    fn write_artifact_versions(
        &self,
        artifact_name: &str,
        versions: &Versions,
    ) -> Result<(), Error> {
        self.backend.create_file(
            &path::artifact::versions(artifact_name),
            sane::to_string(versions)?,
        )
    }

    fn write_artifact(
        &self,
        artifact_name: &str,
        version: &Version,
        artifact: &Artifact,
    ) -> Result<(), Error> {
        self.backend.create_file(
            &path::artifact::artifact(artifact_name, version),
            sane::to_string(artifact)?,
        )
    }

    fn write_latest(&self, artifact_name: &str, version: &Version) -> Result<(), Error> {
        self.backend.create_file(
            &path::artifact::latest(artifact_name),
            sane::to_string(&Latest {
                latest_version: version.clone(),
            })?,
        )
    }

    /// Initialize artifact repo, do nothing if the artifact repo is already initialized
    fn init_artifact(&self, artifact_name: &str) -> Result<Versions, Error> {
        validate_artifact_name(artifact_name)?;
        match self.list_artifact_versions(artifact_name) {
            Ok(versions) => Ok(versions),
            Err(_) => {
                // init the repo
                let mut artifacts = self.init()?;
                // write new versions file
                let new_versions = Versions::new();
                self.write_artifact_versions(artifact_name, &new_versions)?;
                // register artifact
                artifacts.artifacts.push(artifact_name.into());
                self.write_artifacts(&artifacts)?;
                Ok(new_versions)
            }
        }
    }

    pub fn list_artifacts(&self) -> Result<Artifacts, Error> {
        Ok(sane::from_str::<Artifacts>(
            &self.backend.read_file(path::artifacts())?,
        )?)
    }

    pub fn list_artifact_versions(&self, artifact_name: &str) -> Result<Versions, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = path::artifact::versions(artifact_name);
        Ok(sane::from_str::<Versions>(&self.backend.read_file(&path)?)?)
    }

    pub fn latest_artifact_versions(&self, artifact_name: &str) -> Result<Version, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = path::artifact::latest(artifact_name);
        Ok(sane::from_str::<Latest>(&self.backend.read_file(&path)?)?.latest_version)
    }

    pub fn get_artifact(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
    ) -> Result<Artifact, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = path::artifact::artifact(artifact_name, artifact_version);
        Ok(sane::from_str::<Artifact>(&self.backend.read_file(&path)?)?)
    }

    pub fn push_artifact<P: AsRef<Path>>(
        &self,
        artifact_name: &str,
        version: &Version,
        files: &[P],
    ) -> Result<Artifact, Error> {
        // Compute sums & signature
        let mut versions = self.init_artifact(artifact_name)?;
        if versions.versions.contains(&version) {
            Err(RepositoryError::ArtifactVersionAlreadyExists)?;
        }

        let latest = versions
            .versions
            .iter()
            .fold(true, |is_latest, existing_version| {
                existing_version < version
            });

        let publish_algorithm = self.config.get_publish_algorithm()?;

        // create the "Artifact": computes hash & signatures
        let mut digests = Vec::new();
        let mut filenames = Vec::new();
        let mut to_sign = String::new();
        for file in files {
            let digest = base64::encode(&crypto::digest_file(
                file,
                publish_algorithm.checksum_method.algorithm(),
            )?);
            let filename = file
                .as_ref()
                .iter()
                .last()
                .unwrap() // this cannot fail ;)
                .to_string_lossy();

            // construct string to sign
            to_sign.push_str(&digest);
            to_sign.push_str(&filename);

            filenames.push(filename);
            digests.push(digest);
        }
        let signature = base64::encode(&publish_algorithm.signer.sign(to_sign.as_bytes())?);

        let artifact = Artifact {
            signature_method: publish_algorithm.signer.signature_method(),
            version: version.clone(),
            files: filenames
                .iter()
                .zip(digests.into_iter())
                .map(|(filename, digest)| metadata::File {
                    checksum_method: publish_algorithm.checksum_method,
                    checksum: digest,
                    name: filename.to_string(),
                })
                .collect(),
            signature,
        };

        for (file, filename) in files.iter().zip(filenames.iter()) {
            let mut local_path = PathBuf::new();
            local_path.push(file);
            self.backend.push_file(
                local_path,
                &path::artifact::artifact_file(artifact_name, version, filename),
            )?;
        }

        self.write_artifact(artifact_name, version, &artifact)?;
        versions.versions.push(version.clone());
        self.write_artifact_versions(artifact_name, &versions)?;
        if latest {
            self.write_latest(artifact_name, version)?;
        }

        Ok(artifact)
    }

    pub fn pull_artifact<P: AsRef<Path>>(
        &self,
        artifact_name: &str,
        artifact_version: &str,
        destination_dir: P,
    ) -> Result<Artifact, Error> {
        unimplemented!()
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;
    use semver::Version;

    #[test]
    fn validate_artifact_name() {
        super::validate_artifact_name("foo").unwrap();
        super::validate_artifact_name("-f_54321Af.fesoo").unwrap();
        assert!(super::validate_artifact_name(" ").is_err());
        assert!(super::validate_artifact_name("").is_err());
        assert!(super::validate_artifact_name("someé").is_err());
    }

    #[test]
    fn integration_test_file_backend() {
        let config = Config::read_from_file("./test/test-file-backend-config.sane").unwrap();
        clean_file_bck_dir();
        let repo = super::Repository::new(config);
        repo.push_artifact(
            "binrep",
            &Version::parse("1.2.3-alpha").unwrap(),
            &vec![
                "Cargo.toml",
                "./src/lib.rs",
                "test/test-file-backend-config.sane",
            ],
        )
        .unwrap();
    }

    #[allow(unused_must_use)]
    fn clean_file_bck_dir() {
        std::fs::remove_dir_all("./test-file-backend-repo");
    }
}
