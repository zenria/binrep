//! Read a config file containing a list of binrep operation to perform.

#![allow(dead_code)]
#![allow(unused_variables)]
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep::binrep::Binrep;
use binrep::config_resolver::resolve_config;
use serde::Deserialize;

#[derive(StructOpt)]
struct Opt {
    /// Configuration file, if not specified, default to ~/.binrep/config.sane and /etc/binrep/config.sane
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_file: Option<PathBuf>,
    /// batch configuration file, if not provided default to  ~/.binrep/batch.sane
    /// and /etc/binrep/batch.sane
    batch_configuration_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct SyncOperation {
    #[serde(rename = "name")]
    pub artifact_name: String,
    #[serde(rename = "version")]
    pub version_req: String,
    #[serde(rename = "destination")]
    pub destination_dir: String,
    pub exec: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BatchConfig {
    #[serde(rename = "sync")]
    sync_operations: Vec<SyncOperation>,
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
    batch::sync(&binrep, batch_config.sync_operations)?;
    Ok(())
}

mod batch {
    use binrep::binrep::{Binrep, SyncStatus};
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
                version_req: VersionReq::parse(&value.version_req)?,
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
        "#;
        sane::from_str::<BatchConfig>(c).unwrap();
        // test empty config

        sane::from_str::<BatchConfig>("sync=[]").unwrap();
    }
}
