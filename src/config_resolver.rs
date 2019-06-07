use crate::config::Config;
use failure::{Error, Fail};
use std::path::PathBuf;

const DEFAULT_CONFIG_LOCATION: &[&str] = &["~/.binrep/config.sane", "/etc/binrep/config.sane"];

#[derive(Fail, Debug)]
#[fail(display = "No config file provided nor config file found in default location")]
pub struct NoConfigFileError;

pub fn resolve_config(config: Option<PathBuf>) -> Result<Config, Error> {
    config
        .into_iter()
        .chain(
            DEFAULT_CONFIG_LOCATION
                .iter()
                .map(|loc| shellexpand::tilde(*loc))
                .map(|default_location| PathBuf::from(default_location.into_owned())),
        )
        .filter(|loc| loc.exists())
        .nth(0)
        .map(|loc| Config::read_from_file(loc))
        .unwrap_or(Err(NoConfigFileError.into()))
}
