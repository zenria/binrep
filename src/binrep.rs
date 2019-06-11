//! High level binrep API
use crate::config_resolver::resolve_config;
use crate::metadata::*;
use crate::repository::Repository;
use failure::Error;
use semver::Version;
use std::path::Path;

struct Binrep {
    repository: Repository,
}

impl Binrep {
    fn new<P: AsRef<Path>>(config_path: Option<P>) -> Result<Binrep, Error> {
        let config = resolve_config(config_path)?;
        let repository = Repository::new(config);
        Ok(Self { repository })
    }

    fn push<P: AsRef<Path>>(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
        files: &[P],
    ) -> Result<Artifact, Error> {
        self.repository
            .push_artifact(artifact_name, artifact_version, files)
    }

    pub fn pull<P: AsRef<Path>>(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
        destination_dir: P,
        overwrite_dest: bool,
    ) -> Result<Artifact, Error> {
        self.repository.pull_artifact(
            artifact_name,
            artifact_version,
            destination_dir,
            overwrite_dest,
        )
    }

    pub fn sync<P: AsRef<Path>>(
        &self,
        artifact_name: &str,
        artifact_version: &Version,
        destination_dir: P,
        overwrite_dest: bool,
    ) -> Result<Artifact, Error> {
        self.repository.pull_artifact(
            artifact_name,
            artifact_version,
            destination_dir,
            overwrite_dest,
        )
    }
}

mod sync {
    use semver::Version;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
    struct SyncMetadata {
        version: Version,
        last_updated: String,
    }

}
