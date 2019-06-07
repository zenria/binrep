use crate::metadata::{ChecksumMethod, SignatureMethod};
use failure::{Error, Fail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io;
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
    backend_type: BackendType,
    root: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    backend: Backend,
    signature_method: SignatureMethod,
    checksum_method: ChecksumMethod,
    hmac_sha256_keys: Option<HashMap<String, String>>,
    hmac_sha256_signing_key: Option<String>,
}

#[derive(Debug, Fail)]
pub enum ConfigValidationError {
    #[fail(display = "hmac key reference '{}' not found", key_id)]
    HmacSigningKeyNotFound { key_id: String },
    #[fail(display = "no hmac keys configured!")]
    NoHmacKeysConfigured,
    #[fail(display = "no hmac signing keys configured!")]
    NoHmacSigningKeysConfigured,
}

impl Config {
    fn validate_hmac_sha256_signature_method(&self) -> Result<(), ConfigValidationError> {
        match &self.hmac_sha256_keys {
            None => Err(ConfigValidationError::NoHmacKeysConfigured),
            Some(hmac_sha256_keys) => match &self.hmac_sha256_signing_key {
                None => Err(ConfigValidationError::NoHmacSigningKeysConfigured),
                Some(key_id) => {
                    if !hmac_sha256_keys.contains_key(key_id) {
                        Err(ConfigValidationError::HmacSigningKeyNotFound {
                            key_id: key_id.clone(),
                        })
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    pub fn validate_publish_signature_method(&self) -> Result<(), ConfigValidationError> {
        match &self.signature_method {
            SignatureMethod::HmacSha256 => self.validate_hmac_sha256_signature_method(),
            _ => unimplemented!(),
        }
    }

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
        config.validate_publish_signature_method().unwrap();
    }
}
