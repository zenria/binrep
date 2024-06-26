use crate::backend::file_backend::FileBackend;
use crate::backend::s3_backend::S3Backend;
use crate::backend::{Backend, BackendError};
use crate::config::{BackendType, Config};
use crate::crypto::Signer;
use crate::metadata::{Artifact, Artifacts, ChecksumMethod, Signature, SignatureMethod, Versions};
use crate::path::artifacts;
use anyhow::Error;
use core::borrow::Borrow;
use futures::{StreamExt, TryStreamExt};
use ring::digest::{Algorithm, Digest};
use semver::Version;
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, tempdir_in, TempDir};

use crate::crypto;
use crate::file_utils;
use crate::file_utils::{mv, path_concat2};
use crate::metadata;
use crate::path;
use crate::progress::ProgressReporter;

/// Low level API to the repository
pub struct Repository<T: ProgressReporter> {
    backend: Box<dyn Backend<T>>,
    config: Config,
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Wrong artifact naming, only alphanumeric characters and -_. are allowed")]
    ArtifactNameError,
    #[error("Artifact version already exists")]
    ArtifactVersionAlreadyExists,
    #[error("Wrong artifact signature")]
    WrongArtifactSignature,
    #[error("Wrong file checksum for {0}")]
    WrongFileChecksum(String),
    #[error("Destination file already exists {0}")]
    DestinationFileAlreadyExists(String),
    #[error("File backend root is missing")]
    MissingFileBackendRoot,
    #[error("Missing S3 configuration")]
    MissingS3Configuration,
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

impl<T> Repository<T>
where
    T: ProgressReporter + 'static,
    T::Output: Send + Sync + 'static,
{
    pub fn new(config: Config) -> Result<Self, Error> {
        // Construct the backend
        let backend: Box<dyn Backend<T>> = match &config.backend.backend_type {
            BackendType::File => Box::new(FileBackend::<T>::new(
                &config
                    .backend
                    .file_backend_opt
                    .as_ref()
                    .ok_or(RepositoryError::MissingFileBackendRoot)?
                    .root,
            )),
            BackendType::S3 => Box::new(S3Backend::<T>::new(
                config
                    .backend
                    .s3_backend_opt
                    .as_ref()
                    .ok_or(RepositoryError::MissingS3Configuration)?,
            )?),
        };
        Ok(Self { backend, config })
    }

    /// Initialize the repository, do nothing if the repository is already initialized.
    ///
    /// Always returns the Artifacts list
    async fn init(&mut self) -> Result<Artifacts, Error> {
        match self.list_artifacts().await {
            Ok(artifacts) => Ok(artifacts),
            Err(_) => {
                let new_artifacts = Artifacts::new();
                self.write_artifacts(&new_artifacts).await?;
                Ok(new_artifacts)
            }
        }
    }

    async fn write_artifacts(&mut self, artifacts: &Artifacts) -> Result<(), Error> {
        info!("writing {}", path::artifacts());
        Ok(self
            .backend
            .create_file(path::artifacts(), sane::to_string(artifacts)?)
            .await?)
    }

    async fn write_artifact_versions(
        &mut self,
        artifact_name: &str,
        versions: &Versions,
    ) -> Result<(), Error> {
        let versions_path = path::artifact::versions(artifact_name);
        info!("writing {}", versions_path);
        Ok(self
            .backend
            .create_file(&versions_path, sane::to_string(versions)?)
            .await?)
    }

    async fn write_artifact(
        &mut self,
        artifact_name: &str,
        version: &Version,
        artifact: &Artifact,
    ) -> Result<(), Error> {
        let artifact_path = path::artifact::artifact(artifact_name, version);
        info!("writing {}", artifact_path);
        Ok(self
            .backend
            .create_file(&artifact_path, sane::to_string(artifact)?)
            .await?)
    }

    /// Initialize artifact repo, do nothing if the artifact repo is already initialized
    async fn init_artifact(&mut self, artifact_name: &str) -> Result<Versions, Error> {
        validate_artifact_name(artifact_name)?;
        match self.list_artifact_versions(artifact_name).await {
            Ok(versions) => Ok(versions),
            Err(e) => {
                // check if the underlying error is a resource not found error meaning
                // the artifact/version.sane does not exists on the backend.
                // this avoid writing an empty version list file if the error is some network error...
                match e.downcast::<BackendError>()? {
                    BackendError::ResourceNotFound => {
                        info!("initializing new artifact {}", artifact_name);
                        // init the repo
                        let mut artifacts = self.init().await?;
                        // write new versions file
                        let new_versions = Versions::new();
                        self.write_artifact_versions(artifact_name, &new_versions)
                            .await?;
                        // register artifact
                        artifacts.artifacts.push(artifact_name.into());
                        self.write_artifacts(&artifacts).await?;
                        Ok(new_versions)
                    }
                    e => Err(e)?,
                }
            }
        }
    }

    pub async fn list_artifacts(&mut self) -> Result<Artifacts, Error> {
        let artifacts_path = path::artifacts();
        info!("Reading {}", artifacts_path);
        Ok(sane::from_str::<Artifacts>(
            &self.backend.read_file(artifacts_path).await?,
        )?)
    }

    pub async fn list_artifact_versions(&mut self, artifact_name: &str) -> Result<Versions, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = path::artifact::versions(artifact_name);
        info!("Reading {}", path);
        Ok(sane::from_str::<Versions>(
            &self.backend.read_file(&path).await?,
        )?)
    }

    pub async fn get_artifact(
        &mut self,
        artifact_name: &str,
        artifact_version: &Version,
    ) -> Result<Artifact, Error> {
        validate_artifact_name(artifact_name)?;

        let path: String = path::artifact::artifact(artifact_name, artifact_version);
        info!("Reading {}", path);
        let ret = sane::from_str::<Artifact>(&self.backend.read_file(&path).await?)?;
        if !ret.verify_signature(&self.config)? {
            Err(RepositoryError::WrongArtifactSignature)?;
        }
        Ok(ret)
    }

    pub async fn push_artifact<P: AsRef<Path>>(
        &mut self,
        artifact_name: &str,
        version: &Version,
        files: &[P],
    ) -> Result<Artifact, Error> {
        // Compute sums & signature
        let mut versions = self.init_artifact(artifact_name).await?;
        if versions.versions.contains(&version) {
            Err(RepositoryError::ArtifactVersionAlreadyExists)?;
        }

        let publish_algorithm = self.config.get_publish_algorithm()?;

        // create the "Artifact": computes hash & signatures
        let mut digests = Vec::new();
        let mut filenames = Vec::new();
        let mut unix_mode = Vec::new();
        let mut to_sign = String::new();
        for file in files {
            let digest = data_encoding::BASE64.encode(
                crypto::digest_file(file, publish_algorithm.checksum_method.algorithm())?.as_ref(),
            );
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

            let meta = std::fs::metadata(file)?;
            let permissions = meta.permissions();
            unix_mode.push(Some(permissions.mode() & 0o777))
        }
        let signature = Signature {
            key_id: publish_algorithm.signer.key_id(),
            signature_method: publish_algorithm.signer.signature_method(),
            signature: data_encoding::BASE64
                .encode(&publish_algorithm.signer.sign(to_sign.as_bytes())?),
        };

        let artifact = Artifact {
            version: version.clone(),
            files: filenames
                .iter()
                .zip(digests.into_iter())
                .zip(unix_mode)
                .map(|((filename, digest), unix_mode)| metadata::File {
                    checksum_method: publish_algorithm.checksum_method,
                    checksum: digest,
                    name: filename.to_string(),
                    unix_mode,
                })
                .collect(),
            signature,
        };

        for (file, filename) in files.iter().zip(filenames.iter()) {
            let local_path = PathBuf::from(file.as_ref());
            self.backend
                .push_file(
                    local_path,
                    &path::artifact::artifact_file(artifact_name, version, filename),
                )
                .await?;
        }

        self.write_artifact(artifact_name, version, &artifact)
            .await?;
        versions.versions.push(version.clone());
        self.write_artifact_versions(artifact_name, &versions)
            .await?;

        Ok(artifact)
    }

    pub async fn pull_artifact<P: AsRef<Path>>(
        &mut self,
        artifact_name: &str,
        artifact_version: &Version,
        destination_dir: P,
        overwrite_dest: bool,
    ) -> Result<Artifact, Error> {
        // First: download to a temporary dir,
        // then verify checksum
        // then move to final destination

        let artifact = self.get_artifact(artifact_name, artifact_version).await?;

        file_utils::mkdirs(&destination_dir)?;

        let tmp_dir = tempdir_in(&destination_dir)?;

        let mut temporary_file_paths: Vec<PathBuf> = Vec::new();
        for file in &artifact.files {
            temporary_file_paths.push(
                self.copy_to_tmpdir(&artifact_name, artifact_version, file, &tmp_dir)
                    .await?,
            );
        }

        // all files are downloaded with checksum been verified,
        // move them to the final destination
        let mut dest_path = PathBuf::new();
        dest_path.push(destination_dir);

        // check file presence
        let dest_file_paths =
            artifact
                .files
                .iter()
                .try_fold(Vec::new(), |mut paths, file| -> Result<_, Error> {
                    let dest_file_path = path_concat2(&dest_path, &file.name);
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
            .try_for_each(|(src, dst)| mv(src, dst))?;

        Ok(artifact)
    }

    async fn copy_to_tmpdir<P: AsRef<Path>>(
        &mut self,
        artifact_name: &str,
        artifact_version: &Version,
        file: &metadata::File,
        tmp_dir: P,
    ) -> Result<PathBuf, Error> {
        let dest_path = path_concat2(&tmp_dir, &file.name);
        info!("Pulling {} to {}", file.name, dest_path.to_string_lossy());
        self.backend
            .pull_file(
                &path::artifact::artifact_file(artifact_name, artifact_version, &file.name),
                dest_path.clone(),
            )
            .await?;

        if let Some(unix_mode) = file.unix_mode {
            let metadata = std::fs::metadata(&dest_path)?;
            let mut permissions = metadata.permissions();
            permissions.set_mode(unix_mode & 0o777);
            std::fs::set_permissions(&dest_path, permissions)?;
        }

        // let's checksum the file.
        let digest = data_encoding::BASE64.encode(
            crypto::digest_file(dest_path.clone(), file.checksum_method.algorithm())?.as_ref(),
        );
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
    use crate::progress::NOOPProgress;
    use semver::Version;

    #[test]
    fn validate_artifact_name() {
        super::validate_artifact_name("foo").unwrap();
        super::validate_artifact_name("-f_54321Af.fesoo").unwrap();
        assert!(super::validate_artifact_name(" ").is_err());
        assert!(super::validate_artifact_name("").is_err());
        assert!(super::validate_artifact_name("someé").is_err());
    }

    #[tokio::test]
    async fn integration_test_file_backend() {
        let config = Config::create_file_test_config();
        let mut repo = super::Repository::<NOOPProgress>::new(config).unwrap();
        repo.push_artifact(
            "binrep",
            &Version::parse("1.2.3-alpha").unwrap(),
            &vec!["Cargo.toml", "./src/lib.rs"],
        )
        .await
        .unwrap();
        repo.push_artifact(
            "binrep",
            &Version::parse("1.2.1").unwrap(),
            &vec!["./src/backend/mod.rs", "./src/lib.rs"],
        )
        .await
        .unwrap();

        assert_eq!(
            vec!["binrep".to_string()],
            repo.list_artifacts().await.unwrap().artifacts
        );

        let versions = repo
            .list_artifact_versions("binrep")
            .await
            .unwrap()
            .versions;
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
            .await
            .is_err());

        repo.get_artifact("binrep", &Version::parse("1.2.1").unwrap())
            .await
            .unwrap();

        let pull_dir = tempfile::tempdir().unwrap();

        repo.pull_artifact(
            "binrep",
            &Version::parse("1.2.1").unwrap(),
            pull_dir.path(),
            false,
        )
        .await
        .unwrap();
        assert!(repo
            .pull_artifact(
                "binrep",
                &Version::parse("1.2.1").unwrap(),
                pull_dir.path(),
                false,
            )
            .await
            .is_err());
        repo.pull_artifact(
            "binrep",
            &Version::parse("1.2.1").unwrap(),
            pull_dir.path(),
            true,
        )
        .await
        .unwrap();
    }
}
