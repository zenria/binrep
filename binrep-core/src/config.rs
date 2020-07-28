use crate::file_utils;
use crate::metadata::{ChecksumMethod, SignatureMethod};
use failure::{Error, Fail};
use rusoto_core::Region;
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
    #[serde(flatten)]
    pub file_backend_opt: Option<FileBackendOpt>,
    #[serde(flatten)]
    pub s3_backend_opt: Option<S3BackendOpt>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileBackendOpt {
    pub root: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct S3BackendOpt {
    pub bucket: String,
    pub region: String,
    pub profile: Option<String>,
    pub request_timeout_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublishParameters {
    pub signature_method: SignatureMethod,
    pub checksum_method: ChecksumMethod,
    pub hmac_signing_key: Option<String>,
    pub ed25519_signing_key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub backend: Backend,
    pub publish_parameters: Option<PublishParameters>,
    pub hmac_keys: Option<HashMap<String, String>>,
    pub ed25519_keys: Option<HashMap<String, ED25519Key>>,
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum ED25519Key {
    // pkcs8 contains both public & private keys
    SignAndVerify { pkcs8: String },
    Verify { public_key: String },
}

#[derive(Debug, Fail)]
pub enum ConfigValidationError {
    #[fail(display = "ED25519 key reference '{}' not found", key_id)]
    ED25519SigningKeyNotFound { key_id: String },
    #[fail(display = "no ED25519 keys configured!")]
    NoED25519KeysConfigured,
    #[fail(display = "no ED25519 signing key configured!")]
    NoED25519SigningKeyConfigured,
    #[fail(display = "Malformed ED25519 key '{}'", cause)]
    MalformedED25519Key { cause: String },

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
        file_utils::read_sane_from_file(file)
    }
    #[cfg(test)]
    pub fn create_file_test_config() -> Config {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend {
            backend_type: BackendType::File,
            file_backend_opt: Some(FileBackendOpt {
                root: dir.into_path().to_string_lossy().into(),
            }),
            s3_backend_opt: None,
        };
        let mut hmac_keys = HashMap::new();
        hmac_keys.insert(
            "test".to_string(),
            "Ia5m317AYNN9V6Xz8ISm/NqfvHUrTJIN7OxGtWezx9eG/sA/RWT/xP/VwZ8ELaQ3".to_string(),
        );

        let publish_parameters = Some(PublishParameters {
            signature_method: SignatureMethod::HmacSha384,
            checksum_method: ChecksumMethod::Sha384,
            hmac_signing_key: Some("test".to_string()),
            ed25519_signing_key: None,
        });
        Config {
            backend,
            publish_parameters,
            hmac_keys: Some(hmac_keys),
            ed25519_keys: None,
        }
    }

    pub fn create_file_test_config_ed25519_publish() -> Config {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend {
            backend_type: BackendType::File,
            file_backend_opt: Some(FileBackendOpt {
                root: dir.into_path().to_string_lossy().into(),
            }),
            s3_backend_opt: None,
        };
        let mut ed25519_keys = HashMap::new();
        ed25519_keys.insert(
            "test".to_string(),
            ED25519Key::SignAndVerify {
                pkcs8: "MFMCAQEwBQYDK2VwBCIEIIs/h3QgK0hSPeYJqvNoXARyCgjuLTwMVOPdtlK3HYXBoSMDIQD5s1MF9Sw8VK4vxtF9/bQ+AwJjMFMY5xQsc9qJ4ULm3A==".to_string()
            }

        );
        let publish_parameters = Some(PublishParameters {
            signature_method: SignatureMethod::ED25519,
            checksum_method: ChecksumMethod::Sha384,
            hmac_signing_key: None,
            ed25519_signing_key: Some("test".to_string()),
        });
        Config {
            backend,
            publish_parameters,
            hmac_keys: None,
            ed25519_keys: Some(ed25519_keys),
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn parse_sample_config() {
        let config = super::Config::read_from_file("config.sane").unwrap();
        config.get_publish_algorithm().unwrap();
        super::Config::read_from_file("config-s3.sane")
            .unwrap()
            .backend
            .s3_backend_opt
            .unwrap();
    }
}
