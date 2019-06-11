//! High level binrep API
use crate::config_resolver::resolve_config;
use crate::repository::Repository;
use failure::Error;
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
}
