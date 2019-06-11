//! High level binrep API
use crate::config::Config;
use crate::config_resolver::resolve_config;
use crate::file_utils;
use crate::metadata::*;
use crate::repository::Repository;
use failure::{Error, Fail};
use semver::{Version, VersionReq};
use std::path::Path;

pub struct Binrep {
    repository: Repository,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SyncStatus {
    UpToDate,
    Updated,
}

#[derive(Debug)]
pub struct SyncResult {
    artifact: Artifact,
    status: SyncStatus,
}

#[derive(Fail, Debug)]
#[fail(display = "No version matching the requirement")]
struct NoVersionMatching;

impl Binrep {
    pub fn new<P: AsRef<Path>>(config_path: Option<P>) -> Result<Binrep, Error> {
        let config = resolve_config(config_path)?;
        Ok(Self::from_config(config))
    }

    pub fn from_config(config: Config) -> Binrep {
        let repository = Repository::new(config);
        Self { repository }
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
        version_req: Option<&VersionReq>,
        destination_dir: P,
    ) -> Result<SyncResult, Error> {
        file_utils::mkdirs(&destination_dir)?;

        let latest = match version_req {
            None => self.repository.latest_artifact_versions(artifact_name)?,
            Some(version_req) => {
                let mut matching_versions: Vec<Version> = self
                    .repository
                    .list_artifact_versions(artifact_name)?
                    .versions
                    .into_iter()
                    .filter(|version| version_req.matches(version))
                    .collect();
                matching_versions.sort();
                match matching_versions.into_iter().last() {
                    Some(max_matching_version) => max_matching_version,
                    None => Err(NoVersionMatching)?,
                }
            }
        };

        let sync_meta = sync::read_meta(artifact_name, &destination_dir)?;
        match &sync_meta {
            Some(meta) if meta.version == latest => {
                info!("Already the latest version");
                let artifact = self.repository.get_artifact(artifact_name, &latest)?;
                Ok(SyncResult {
                    artifact,
                    status: SyncStatus::UpToDate,
                })
            }
            _ => {
                let artifact = self.repository.pull_artifact(
                    artifact_name,
                    &latest,
                    &destination_dir,
                    true,
                )?;

                sync::write_meta(
                    artifact_name,
                    &destination_dir,
                    &sync::SyncMetadata::new(latest),
                )?;

                Ok(SyncResult {
                    artifact,
                    status: SyncStatus::Updated,
                })
            }
        }
    }
}

mod sync {
    use crate::file_utils;
    use chrono::prelude::*;
    use failure::Error;
    use semver::Version;
    use serde::{Deserialize, Serialize};
    use std::fs::File;
    use std::io::{ErrorKind, Write};
    use std::path::{Path, PathBuf};

    #[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
    pub struct SyncMetadata {
        pub version: Version,
        last_updated: String,
    }

    impl SyncMetadata {
        pub fn new(version: Version) -> Self {
            Self {
                version,
                last_updated: Utc::now().to_rfc3339(),
            }
        }
    }

    fn get_meta_path<P: AsRef<Path>>(artifact_name: &str, dir: P) -> PathBuf {
        let mut ret = PathBuf::from(dir.as_ref());
        let filename: String = vec![".", artifact_name, "_sync.sane"].into_iter().collect();
        ret.push(filename);
        ret
    }

    pub fn read_meta<P: AsRef<Path>>(
        artifact_name: &str,
        dir: P,
    ) -> Result<Option<SyncMetadata>, Error> {
        let meta_file_path = get_meta_path(artifact_name, dir);
        match std::fs::metadata(&meta_file_path) {
            Ok(_) => Ok(Some(file_utils::read_sane_from_file(&meta_file_path)?)),
            Err(ioe) => match ioe.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(ioe)?,
            },
        }
    }

    pub fn write_meta<P: AsRef<Path>>(
        artifact_name: &str,
        dir: P,
        meta: &SyncMetadata,
    ) -> Result<(), Error> {
        file_utils::write_sane_to_file(get_meta_path(artifact_name, dir), meta)
    }

}

#[cfg(test)]
mod test {
    use super::*;

    static ANAME: &'static str = "binrep";

    #[test]
    fn test_binrep() {
        let br = Binrep::from_config(Config::create_file_test_config());
        let v1 = Version::parse("1.0.0").unwrap();
        let v12 = Version::parse("1.2.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();

        br.push(ANAME, &v1, &vec!["Cargo.toml"]).unwrap();

        let dest_sync = tempfile::tempdir().unwrap();

        let sr = br.sync(ANAME, None, &dest_sync).unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v1, sr.artifact.version);

        let sr = br.sync(ANAME, None, &dest_sync).unwrap();
        assert_eq!(SyncStatus::UpToDate, sr.status);
        assert_eq!(v1, sr.artifact.version);

        br.push(ANAME, &v12, &vec!["Cargo.toml"]).unwrap();
        br.push(ANAME, &v2, &vec!["Cargo.toml"]).unwrap();

        let sr = br.sync(ANAME, None, &dest_sync).unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v2, sr.artifact.version);

        let sr = br.sync(ANAME, None, &dest_sync).unwrap();
        assert_eq!(SyncStatus::UpToDate, sr.status);
        assert_eq!(v2, sr.artifact.version);

        // try downgrading to 1.2.x
        let sr = br
            .sync(ANAME, Some(&VersionReq::parse("~1").unwrap()), &dest_sync)
            .unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v12, sr.artifact.version);
        let sr = br
            .sync(ANAME, Some(&VersionReq::parse("~1").unwrap()), &dest_sync)
            .unwrap();
        assert_eq!(SyncStatus::UpToDate, sr.status);
        assert_eq!(v12, sr.artifact.version);

        let sr = br
            .sync(ANAME, Some(&VersionReq::parse("*").unwrap()), &dest_sync)
            .unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v2, sr.artifact.version);
    }
}
