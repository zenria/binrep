//! High level binrep API
use crate::config::Config;
use crate::config_resolver::resolve_config as resolve_any_config;
use crate::file_utils;
use crate::file_utils::{mkdirs, mv, path_concat2, LockFile};
use crate::metadata::*;
use crate::progress::ProgressReporter;
use crate::repository::Repository;
use anyhow::Error;
use fs2::FileExt;
use semver::{Version, VersionReq};
use serde::de::DeserializeOwned;
use slack_hook3::{AttachmentBuilder, Payload, PayloadBuilder, Slack};
use std::fs::metadata;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::{tempdir, tempdir_in};

pub struct Binrep<T: ProgressReporter> {
    repository: Repository<T>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SyncStatus {
    UpToDate,
    Updated,
}

#[derive(Debug)]
pub struct SyncResult {
    pub artifact: Artifact,
    pub status: SyncStatus,
}

#[derive(thiserror::Error, Debug)]
#[error("No version is matching the requirement {version_req}")]
struct NoVersionMatching {
    version_req: VersionReq,
}

pub fn resolve_config<P: AsRef<Path>, D: DeserializeOwned>(
    config_path: &Option<P>,
) -> Result<D, Error> {
    resolve_any_config(&config_path, "config.sane")
}

impl<T> Binrep<T>
where
    T: ProgressReporter + 'static,
    T::Output: Send + Sync + 'static,
{
    pub fn new<P: AsRef<Path>>(config_path: &Option<P>) -> Result<Binrep<T>, Error> {
        let config: Config = resolve_config(config_path)?;
        Self::from_config(config)
    }

    pub fn from_config(config: Config) -> Result<Binrep<T>, Error> {
        let repository = Repository::new(config)?;
        Ok(Self { repository })
    }

    pub async fn list_artifacts(&mut self) -> Result<Artifacts, Error> {
        self.repository.list_artifacts().await
    }

    pub async fn list_artifact_versions(
        &mut self,
        artifact_name: &str,
        version_req: &VersionReq,
    ) -> Result<Vec<Version>, Error> {
        Ok(self
            .repository
            .list_artifact_versions(artifact_name)
            .await?
            .versions
            .into_iter()
            .filter(|v| version_req.matches(v))
            .collect())
    }

    pub async fn artifact(
        &mut self,
        artifact_name: &str,
        artifact_version: &Version,
    ) -> Result<Artifact, Error> {
        self.repository
            .get_artifact(artifact_name, artifact_version)
            .await
    }

    pub async fn push<P: AsRef<Path>>(
        &mut self,
        artifact_name: &str,
        artifact_version: &Version,
        files: &[P],
    ) -> Result<Artifact, Error> {
        self.repository
            .push_artifact(artifact_name, artifact_version, files)
            .await
    }

    pub async fn pull<P: AsRef<Path>>(
        &mut self,
        artifact_name: &str,
        artifact_version: &Version,
        destination_dir: P,
        overwrite_dest: bool,
    ) -> Result<Artifact, Error> {
        self.repository
            .pull_artifact(
                artifact_name,
                artifact_version,
                destination_dir,
                overwrite_dest,
            )
            .await
    }

    pub async fn last_version(
        &mut self,
        artifact_name: &str,
        version_req: &VersionReq,
    ) -> Result<Option<Version>, Error> {
        let mut matching_versions = self
            .list_artifact_versions(artifact_name, version_req)
            .await?;
        matching_versions.sort();
        Ok(matching_versions.into_iter().last())
    }

    pub async fn sync<P: AsRef<Path>>(
        &mut self,
        artifact_name: &str,
        version_req: &VersionReq,
        destination_dir: P,
    ) -> Result<SyncResult, Error> {
        file_utils::mkdirs(&destination_dir)?;

        let latest = match self.last_version(artifact_name, version_req).await? {
            Some(max_matching_version) => max_matching_version,
            None => Err(NoVersionMatching {
                version_req: version_req.clone(),
            })?,
        };

        mkdirs(&destination_dir)?;
        let lock_file_path = path_concat2(
            &destination_dir,
            format!(".{}.binrep-sync.lock", artifact_name),
        );
        let lock_file = LockFile::create_and_lock(lock_file_path)?;

        let sync_meta = sync::read_meta(artifact_name, &destination_dir)?;
        match &sync_meta {
            Some(meta) if meta.artifact.version == latest => {
                info!("Already the latest version");
                Ok(SyncResult {
                    artifact: meta.artifact.clone(), // this is a shitty clone!
                    status: SyncStatus::UpToDate,
                })
            }
            meta => {
                // pull artifact to tempdir
                let temp_sync_dir = tempdir_in(&destination_dir)?;
                let artifact = self
                    .repository
                    .pull_artifact(artifact_name, &latest, &temp_sync_dir, true)
                    .await?;
                // remove existing files if any
                meta.as_ref()
                    .map(|meta| meta.artifact.files.clone())
                    .iter()
                    .flatten()
                    .try_for_each(|file| {
                        let file_path = path_concat2(&destination_dir, &file.name);
                        std::fs::metadata(&file_path)
                            .and_then(|_| std::fs::remove_file(&file_path))
                            .or::<std::io::Error>(Ok(()))
                    })?;
                // move temp file to final destination
                artifact.files.iter().try_for_each(|file| {
                    let src = path_concat2(&temp_sync_dir, &file.name);
                    let dst = path_concat2(&destination_dir, &file.name);
                    mv(src, dst)
                })?;

                info!("Synced to {}", artifact);
                let new_meta = sync::SyncMetadata::new(artifact);
                sync::write_meta(artifact_name, &destination_dir, &new_meta)?;

                Ok(SyncResult {
                    artifact: new_meta.artifact,
                    status: SyncStatus::Updated,
                })
            }
        }
    }
}

mod sync {
    use crate::file_utils;
    use crate::metadata::Artifact;
    use anyhow::Error;
    use chrono::prelude::*;
    use semver::Version;
    use serde::{Deserialize, Serialize};
    use std::fs::File;
    use std::io::{ErrorKind, Write};
    use std::path::{Path, PathBuf};

    #[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
    pub struct SyncMetadata {
        last_updated: String,
        pub artifact: Artifact,
    }

    impl SyncMetadata {
        pub fn new(artifact: Artifact) -> Self {
            Self {
                artifact,
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

pub fn parse_version_req(input: &str) -> Result<VersionReq, Error> {
    Ok(match input {
        v if v == "latest" || v == "any" => VersionReq::STAR,
        v => VersionReq::parse(v)?,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::file_utils::path_concat2;
    use crate::progress::NOOPProgress;
    use semver::Comparator;
    use std::fs::metadata;
    use std::path::PathBuf;
    use tempfile::tempdir;

    static ANAME: &'static str = "binrep";

    #[tokio::test]
    async fn test_binrep() {
        let mut br: Binrep<NOOPProgress> =
            Binrep::from_config(Config::create_file_test_config()).unwrap();
        let v1 = Version::parse("1.0.0").unwrap();
        let v12 = Version::parse("1.2.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();

        br.push(ANAME, &v1, &vec!["Cargo.toml"]).await.unwrap();

        let dest_sync = tempfile::tempdir().unwrap();

        let sr = br.sync(ANAME, &VersionReq::STAR, &dest_sync).await.unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v1, sr.artifact.version);

        let sr = br.sync(ANAME, &VersionReq::STAR, &dest_sync).await.unwrap();
        assert_eq!(SyncStatus::UpToDate, sr.status);
        assert_eq!(v1, sr.artifact.version);

        br.push(ANAME, &v12, &vec!["Cargo.toml"]).await.unwrap();
        br.push(ANAME, &v2, &vec!["Cargo.toml"]).await.unwrap();

        let sr = br.sync(ANAME, &VersionReq::STAR, &dest_sync).await.unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v2, sr.artifact.version);

        let sr = br.sync(ANAME, &VersionReq::STAR, &dest_sync).await.unwrap();
        assert_eq!(SyncStatus::UpToDate, sr.status);
        assert_eq!(v2, sr.artifact.version);

        // try downgrading to 1.2.x
        let sr = br
            .sync(ANAME, &VersionReq::parse("~1").unwrap(), &dest_sync)
            .await
            .unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v12, sr.artifact.version);
        let sr = br
            .sync(ANAME, &VersionReq::parse("~1").unwrap(), &dest_sync)
            .await
            .unwrap();
        assert_eq!(SyncStatus::UpToDate, sr.status);
        assert_eq!(v12, sr.artifact.version);

        let sr = br.sync(ANAME, &VersionReq::STAR, &dest_sync).await.unwrap();
        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(v2, sr.artifact.version);
    }
    #[tokio::test]
    async fn test_alpha() {
        let mut br: Binrep<NOOPProgress> =
            Binrep::from_config(Config::create_file_test_config()).unwrap();
        let valpha = Version::parse("1.0.0-alpha1").unwrap();
        br.push(ANAME, &valpha, &vec!["Cargo.toml"]).await.unwrap();

        let dest_sync = tempfile::tempdir().unwrap();

        let sr = br
            .sync(ANAME, &super::parse_version_req("any").unwrap(), &dest_sync)
            .await
            .expect_err("any version does not matches prerelease");

        let sr = br
            .sync(
                ANAME,
                &super::parse_version_req(">=1.0.0-alph").unwrap(),
                &dest_sync,
            )
            .await
            .expect(">=1.0.0-alph MUST matches 1.0.0-alpha1");

        assert_eq!(SyncStatus::Updated, sr.status);
        assert_eq!(valpha, sr.artifact.version);
    }

    #[tokio::test]
    async fn test_sync_file_presence() {
        fn exact(v: &Version) -> VersionReq {
            VersionReq {
                comparators: vec![Comparator {
                    op: semver::Op::Exact,
                    major: v.major,
                    minor: Some(v.minor),
                    patch: Some(v.patch),
                    pre: v.pre.clone(),
                }],
            }
        }

        let mut br: Binrep<NOOPProgress> =
            Binrep::from_config(Config::create_file_test_config()).unwrap();
        let v1 = Version::parse("1.0.0").unwrap();
        let v12 = Version::parse("1.2.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();

        let artifact_src = tempdir().unwrap();
        let path_v1 = path_concat2(artifact_src.path(), "a-1.zip");
        let path_v2 = path_concat2(artifact_src.path(), "a-2.zip");

        std::fs::File::create(&path_v1).unwrap();
        std::fs::File::create(&path_v2).unwrap();

        br.push("a", &v1, &vec![&path_v1]).await.unwrap();
        br.push("a", &v12, &vec![&path_v1]).await.unwrap();
        br.push("a", &v2, &vec![&path_v2]).await.unwrap();

        let syncdest = tempdir().unwrap();
        let synced_path_v1 = path_concat2(syncdest.path(), "a-1.zip");
        let synced_path_v2 = path_concat2(syncdest.path(), "a-2.zip");

        // sync v1
        assert_eq!(
            SyncStatus::Updated,
            br.sync("a", &exact(&v1), syncdest.path())
                .await
                .unwrap()
                .status,
        );
        assert_path(PathAssertion::File, &synced_path_v1);
        assert_path(PathAssertion::Absent, &synced_path_v2);
        // sync v12
        assert_eq!(
            SyncStatus::Updated,
            br.sync("a", &exact(&v12), syncdest.path())
                .await
                .unwrap()
                .status,
        );
        assert_path(PathAssertion::File, &synced_path_v1);
        assert_path(PathAssertion::Absent, &synced_path_v2);
        // re-sync v12
        assert_eq!(
            SyncStatus::UpToDate,
            br.sync("a", &exact(&v12), syncdest.path())
                .await
                .unwrap()
                .status,
        );
        assert_path(PathAssertion::File, &synced_path_v1);
        assert_path(PathAssertion::Absent, &synced_path_v2);
        // sync "latest"
        assert_eq!(
            SyncStatus::Updated,
            br.sync("a", &VersionReq::STAR, syncdest.path())
                .await
                .unwrap()
                .status,
        );
        assert_path(PathAssertion::Absent, &synced_path_v1);
        assert_path(PathAssertion::File, &synced_path_v2);
    }
    #[derive(Eq, PartialEq, Debug)]
    enum PathAssertion {
        Absent, // absent or do not have the right to read meta
        Dir,
        File,
    }
    fn assert_path<P: AsRef<Path>>(assertion: PathAssertion, path: P) {
        match metadata(path.as_ref()) {
            Err(e) => assert_eq!(PathAssertion::Absent, assertion),
            Ok(meta) => match assertion {
                PathAssertion::File => assert!(meta.is_file()),
                PathAssertion::Dir => assert!(meta.is_dir()),
                PathAssertion::Absent => {
                    panic!("{} is not absent", path.as_ref().to_string_lossy())
                }
            },
        }
    }
}
