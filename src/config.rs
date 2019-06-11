use crate::metadata::{ChecksumMethod, SignatureMethod};
use failure::{Error, Fail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BackendType {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "s3")]
    S3,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Backend {
    #[serde(rename = "type")]
    pub backend_type: BackendType,
    pub root: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublishParameters {
    pub signature_method: SignatureMethod,
    pub checksum_method: ChecksumMethod,
    pub hmac_signing_key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub backend: Backend,
    pub publish_parameters: Option<PublishParameters>,
    pub hmac_keys: Option<HashMap<String, String>>,
}

#[derive(Debug, Fail)]
pub enum ConfigValidationError {
    #[fail(display = "hmac key reference '{}' not found", key_id)]
    HmacSigningKeyNotFound { key_id: String },
    #[fail(display = "no hmac keys configured!")]
    NoHmacKeysConfigured,
    #[fail(display = "no hmac signing keys configured!")]
    NoHmacSigningKeysConfigured,
    #[fail(display = "no publish parameters")]
    NoPublishParameters,
    #[fail(
        display = "found invalid hmac key (needs to be 32/48/64 bytes long base64 encoded) {}",
        _0
    )]
    InvalidHmacKey(String),
    #[fail(display = "invalid base 64 encoded string: {}", _0)]
    InvalidBase64Encoding(String),
}

impl Config {
    pub fn read_from_file<P: AsRef<Path>>(file: P) -> Result<Config, Error> {
        let mut config_file = File::open(&file)?;
        let mut config = String::new();
        config_file.read_to_string(&mut config)?;

        // Parse config file
        Ok(sane::from_str(&config)?)
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn parse_sample_config() {
        let config = super::Config::read_from_file("config.sane").unwrap();
        config.get_publish_algorithm().unwrap();
    }
}
