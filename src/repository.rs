use crate::backend::file_backend::FileBackend;
use crate::backend::Backend;
use crate::config::{BackendType, Config};
use crate::crypto::Signer;
use crate::metadata::{Artifact, Artifacts, ChecksumMethod, Signature, SignatureMethod, Versions};
use crate::path::artifacts;
use core::borrow::Borrow;
use failure::{Error, Fail};
use ring::digest::{Algorithm, Digest};
use semver::Version;
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

use crate::crypto;
use crate::file_utils;
use crate::metadata;
use crate::path;

/// Low level API to the repository
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
    #[fail(display = "Wrong artifact signature")]
    WrongArtifactSignature,
    #[fail(display = "Wrong file checksum for {}", _0)]
    WrongFileChecksum(String),
    #[fail(display = "Destination file already exists {}", _0)]
    DestinationFileAlreadyExists(String),
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

    pub fn get_artifact(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
    ) -> Result<Artifact, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = path::artifact::artifact(artifact_name, artifact_version);
        let ret = sane::from_str::<Artifact>(&self.backend.read_file(&path)?)?;
        if !ret.verify_signature(&self.config)? {
            Err(RepositoryError::WrongArtifactSignature)?;
        }
        Ok(ret)
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
            to_sign.push_str(&filename);
            to_sign.push_str(&digest);

            filenames.push(filename);
            digests.push(digest);
        }
        let signature = Signature {
            key_id: publish_algorithm.signer.key_id(),
            signature_method: publish_algorithm.signer.signature_method(),
            signature: base64::encode(&publish_algorithm.signer.sign(to_sign.as_bytes())?),
        };

        let artifact = Artifact {
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

        Ok(artifact)
    }

    pub fn pull_artifact<P: AsRef<Path>>(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
        destination_dir: P,
        overwrite_dest: bool,
    ) -> Result<Artifact, Error> {
        // First: download to a temporary dir,
        // then verify checksum
        // then move to final destination

        let artifact = self.get_artifact(artifact_name, artifact_version)?;

        let tmp_dir = tempdir()?;

        let temporary_file_paths: Vec<PathBuf> =
            artifact
                .files
                .iter()
                .try_fold(Vec::new(), |mut files, file| -> Result<_, Error> {
                    files.push(self.copy_to_tmpdir(
                        artifact_name,
                        artifact_version,
                        file,
                        &tmp_dir,
                    )?);
                    Ok(files)
                })?;

        // all files are downloaded with checksum been verified,
        // move them to the final destination
        let mut dest_path = PathBuf::new();
        dest_path.push(destination_dir);

        file_utils::mkdirs(&dest_path)?;

        // check file presence
        let dest_file_paths =
            artifact
                .files
                .iter()
                .try_fold(Vec::new(), |mut paths, file| -> Result<_, Error> {
                    let mut dest_file_path = PathBuf::new();
                    dest_file_path.push(&dest_path);
                    dest_file_path.push(&file.name);
                    if let Ok(_) = std::fs::metadata(&dest_file_path) {
                        if !overwrite_dest {
                            // cannot overwrite => error
                            Err(RepositoryError::DestinationFileAlreadyExists(
                                dest_file_path.to_string_lossy().into(),
                            ))?;
                        } else {
                            // delete existing file
                            std::fs::remove_file(&dest_file_path)?;
                        }
                    }
                    paths.push(dest_file_path);
                    Ok(paths)
                })?;

        temporary_file_paths
            .iter()
            .zip(dest_file_paths.iter())
            .try_for_each(|(src, dst)| Self::mv(src, dst))?;

        Ok(artifact)
    }

    fn mv<S: AsRef<Path>, D: AsRef<Path>>(src: S, dst: D) -> Result<(), std::io::Error> {
        match std::fs::rename(src.as_ref(), dst.as_ref()) {
            Ok(_) => Ok(()),
            Err(e) => match e.kind() {
                ErrorKind::Other => std::fs::copy(src, dst).map(|_| ()),
                _ => Err(e),
            },
        }
    }

    fn copy_to_tmpdir(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
        file: &metadata::File,
        tmp_dir: &TempDir,
    ) -> Result<PathBuf, Error> {
        let mut dest_path = PathBuf::new();
        dest_path.push(tmp_dir.path());
        dest_path.push(&file.name);
        self.backend.pull_file(
            &path::artifact::artifact_file(artifact_name, artifact_version, &file.name),
            dest_path.clone(),
        )?;

        // let's checksum the file.
        let digest = base64::encode(&crypto::digest_file(
            dest_path.clone(),
            file.checksum_method.algorithm(),
        )?);
        // verify the checksum
        if digest != file.checksum {
            Err(RepositoryError::WrongFileChecksum(file.name.clone()))?;
        }
        Ok(dest_path)
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
        repo.push_artifact(
            "binrep",
            &Version::parse("1.2.1").unwrap(),
            &vec!["./src/backend/mod.rs", "./src/lib.rs"],
        )
        .unwrap();

        assert_eq!(
            vec!["binrep".to_string()],
            repo.list_artifacts().unwrap().artifacts
        );

        let versions = repo.list_artifact_versions("binrep").unwrap().versions;
        assert_eq!(2, versions.len());
        assert!(versions.contains(&Version::parse("1.2.1").unwrap()));
        assert!(versions.contains(&Version::parse("1.2.3-alpha").unwrap()));

        // cannot push twice the same version
        assert!(repo
            .push_artifact(
                "binrep",
                &Version::parse("1.2.1").unwrap(),
                &vec!["./src/backend/mod.rs", "./src/lib.rs"],
            )
            .is_err());

        repo.get_artifact("binrep", &Version::parse("1.2.1").unwrap())
            .unwrap();

        repo.pull_artifact(
            "binrep",
            &Version::parse("1.2.1").unwrap(),
            "./test-file-backend-repo-pull",
            false,
        )
        .unwrap();
        assert!(repo
            .pull_artifact(
                "binrep",
                &Version::parse("1.2.1").unwrap(),
                "./test-file-backend-repo-pull",
                false,
            )
            .is_err());
        repo.pull_artifact(
            "binrep",
            &Version::parse("1.2.1").unwrap(),
            "./test-file-backend-repo-pull",
            true,
        )
        .unwrap();
    }

    #[allow(unused_must_use)]
    fn clean_file_bck_dir() {
        std::fs::remove_dir_all("./test-file-backend-repo");
        std::fs::remove_dir_all("./test-file-backend-repo-pull");
    }
}
