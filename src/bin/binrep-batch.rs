//! Read a config file containing a list of binrep operation to perform.

#![allow(dead_code)]
#![allow(unused_variables)]
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep::binrep::Binrep;
use binrep::config_resolver::resolve_config;
use binrep::file_utils;
use glob::glob;
use serde::Deserialize;
use serde::Serialize;

use log::debug;

#[derive(StructOpt)]
struct Opt {
    /// Configuration file, if not specified, default to ~/.binrep/config.sane and /etc/binrep/config.sane
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_file: Option<PathBuf>,
    /// batch configuration file, if not provided default to  ~/.binrep/batch.sane
    /// and /etc/binrep/batch.sane
    batch_configuration_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct SyncOperation {
    #[serde(rename = "name")]
    pub artifact_name: String,
    #[serde(rename = "version")]
    pub version_req: String,
    #[serde(rename = "destination")]
    pub destination_dir: String,
    pub exec: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct SlackNotifier {
    pub channel: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct BatchConfig {
    /// eg. includes=/etc/binrep/batch.d/*.sane
    includes: Option<String>,
    #[serde(rename = "sync")]
    sync_operations: Vec<SyncOperation>,
    #[serde(rename = "slack")]
    default_slack_notifier: Option<SlackNotifier>,
}

fn main() {
    env_logger::init();
    let opt = Opt::from_args();
    if let Err(e) = _main(opt) {
        eprintln!("{} - {:?}", e, e);
        std::process::exit(1);
    }
}
fn _main(opt: Opt) -> Result<(), Error> {
    let batch_config: BatchConfig = resolve_config(&opt.batch_configuration_file, "batch.sane")?;
    let binrep = Binrep::new(&opt.config_file)?;

    let operations: Vec<SyncOperation> = batch_config
        .sync_operations
        .into_iter()
        .chain(get_operation_from_includes(batch_config.includes))
        .collect();

    batch::sync(&binrep, operations)?;
    Ok(())
}

fn get_operation_from_includes(includes: Option<String>) -> Vec<SyncOperation> {
    includes
        .map(|includes_path| glob(&includes_path).expect("Failed to read glob pattern"))
        .into_iter()
        .flatten()
        .map(|path| path.unwrap())
        .map(|path| {
            debug!("Reading included config file {:?}", path);
            file_utils::read_sane_from_file::<_, BatchConfig>(path)
                .unwrap()
                .sync_operations
        })
        .flatten()
        .collect()
}

mod batch {
    use binrep::binrep::{parse_version_req, Binrep, SyncStatus};
    use binrep::exec::exec;
    use failure::Error;
    use semver::VersionReq;
    use std::convert::{TryFrom, TryInto};
    use std::path::PathBuf;

    struct SyncOperation {
        artifact_name: String,
        version_req: VersionReq,
        destination_dir: PathBuf,
        command: Option<String>,
    }
    impl TryFrom<super::SyncOperation> for SyncOperation {
        type Error = Error;

        fn try_from(value: super::SyncOperation) -> Result<Self, Self::Error> {
            Ok(SyncOperation {
                artifact_name: value.artifact_name,
                version_req: parse_version_req(&value.version_req)?,
                destination_dir: PathBuf::from(value.destination_dir),
                command: value.exec,
            })
        }
    }

    pub fn sync(binrep: &Binrep, operations: Vec<super::SyncOperation>) -> Result<(), Error> {
        // validate config
        let operations: Vec<SyncOperation> = operations.into_iter().try_fold(
            Vec::new(),
            |mut acc, op| -> Result<Vec<SyncOperation>, Error> {
                acc.push(op.try_into()?);
                Ok(acc)
            },
        )?;
        for operation in operations {
            println!(
                "Syncing {} to {}",
                operation.artifact_name,
                operation.destination_dir.to_string_lossy()
            );
            let result = binrep.sync(
                &operation.artifact_name,
                &operation.version_req,
                &operation.destination_dir,
            )?;
            match &result.status {
                SyncStatus::Updated => {
                    println!("updated: {}", result.artifact);
                    exec(
                        &result.artifact,
                        &operation.destination_dir,
                        &operation.command,
                    )?;
                }
                SyncStatus::UpToDate => {
                    println!("Already the latest version {}", result.artifact.version);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::BatchConfig;
    use crate::{get_operation_from_includes, SyncOperation};
    use binrep::file_utils;

    #[test]
    fn test_config() {
        // sane syntax
        let c = r#"sync = [
            {
                name="binrep",
                version="*",
                destination="/srv/dist/binrep/bin"
            },
            {
                name="binrep-bootstrap",
                version="2",
                destination="/srv/www/binrep-bootstrap",
                exec="echo hello"
            },
        ]"#;
        sane::from_str::<BatchConfig>(c).unwrap();
        // our parser also accepts toml syntax
        let c = r#"[[sync]]
            name="binrep"
            version="*"
            destination="/srv/dist/binrep/bin"
            [[sync]]
            name="binrep-bootstrap"
            version="2"
            destination="/srv/www/binrep-bootstrap"
            exec="echo hello"
            slack={ enabled=true }
        "#;
        sane::from_str::<BatchConfig>(c).unwrap();
        // test empty config

        sane::from_str::<BatchConfig>("sync=[]").unwrap();

        sane::from_str::<BatchConfig>("includes=\"/etc/batch.d/*.sync\"\nsync=[]").unwrap();

        assert_eq!(
            Vec::<SyncOperation>::new(),
            get_operation_from_includes(None)
        );
        assert_eq!(
            Vec::<SyncOperation>::new(),
            get_operation_from_includes(Some("src/non-exising/*.sane".into()))
        );
        let temp_dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Vec::<SyncOperation>::new(),
            get_operation_from_includes(Some(format!(
                "{}/*.sane",
                temp_dir.path().to_string_lossy()
            )))
        );

        let file1 = file_utils::path_concat2(&temp_dir, "coucou.sane");
        let operations1 = BatchConfig {
            sync_operations: vec![SyncOperation {
                artifact_name: "coucou".to_string(),
                version_req: "latest".to_string(),
                destination_dir: "/tmp/abcde".to_string(),
                exec: None,
            }],
            includes: None,
            default_slack_notifier: None,
        };
        file_utils::write_sane_to_file(&file1, &operations1).unwrap();

        let file2 = file_utils::path_concat2(&temp_dir, "coucou2.sane");
        let operations2 = BatchConfig {
            sync_operations: vec![
                SyncOperation {
                    artifact_name: "coucou1".to_string(),
                    version_req: "1.3.0".to_string(),
                    destination_dir: "/tmp/abcdef".to_string(),
                    exec: None,
                },
                SyncOperation {
                    artifact_name: "coucou2".to_string(),
                    version_req: "1.0.3".to_string(),
                    destination_dir: "/tmp/abcdsdsdef".to_string(),
                    exec: None,
                },
            ],
            includes: None,
            default_slack_notifier: None,
        };
        file_utils::write_sane_to_file(&file2, &operations2).unwrap();

        assert_eq!(
            operations1
                .sync_operations
                .into_iter()
                .chain(operations2.sync_operations.into_iter())
                .collect::<Vec<_>>(),
            get_operation_from_includes(Some(format!(
                "{}/*.sane",
                temp_dir.path().to_string_lossy()
            )))
        );
    }
}
