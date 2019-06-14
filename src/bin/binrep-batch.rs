//! Read a config file containing a list of binrep operation to perform.

#![allow(dead_code)]
#![allow(unused_variables)]
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep::binrep::{Binrep, SyncStatus};
use binrep::config::Config;
use binrep::config_resolver::resolve_config;
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::fmt::Display;

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
struct SyncOperation {
    #[serde(rename = "name")]
    artifact_name: String,
    #[serde(rename = "version")]
    version_req: String,
    #[serde(rename = "destination")]
    destination_dir: String,
}

#[derive(Debug, Deserialize)]
struct BatchConfig {
    #[serde(rename = "sync")]
    sync_operation: Vec<SyncOperation>,
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
    let config: Config = resolve_config(&opt.config_file, "config.sane")?;
    let batch_config: BatchConfig = resolve_config(&opt.batch_configuration_file, "batch.sane")?;
    Ok(())
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
                destination="/srv/www/binrep-bootstrap"
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
        "#;
        sane::from_str::<BatchConfig>(c).unwrap();
    }
}
