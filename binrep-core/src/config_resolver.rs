use crate::config::Config;
use crate::file_utils;
use anyhow::Error;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::string::ToString;

const DEFAULT_CONFIG_LOCATION: &[&str] = &["~/.binrep/", "/etc/binrep/"];

#[derive(thiserror::Error, Debug)]
#[error("No config file provided nor {0} file found in default locations")]
pub struct NoConfigFileError(String);

pub fn resolve_config<P: AsRef<Path>, T: AsRef<Path>, D: DeserializeOwned>(
    provided_config: &Option<P>,
    name: T,
) -> Result<D, Error> {
    provided_config
        .as_ref()
        .map(|path| PathBuf::from(path.as_ref()))
        .into_iter()
        .chain(
            DEFAULT_CONFIG_LOCATION
                .iter()
                .map(|loc| shellexpand::tilde(*loc))
                .map(|loc| file_utils::path_concat2(loc.into_owned(), &name)),
        )
        .filter(|loc| loc.exists())
        .nth(0)
        .map(|loc| file_utils::read_sane_from_file(loc))
        .unwrap_or(Err(NoConfigFileError(
            name.as_ref().to_string_lossy().into(),
        )
        .into()))
}
