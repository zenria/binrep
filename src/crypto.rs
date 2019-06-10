use crate::config::Config;
use crate::config::ConfigValidationError;
use crate::config::PublishParameters;
use crate::metadata::{ChecksumMethod, SignatureMethod};
use failure::Error;
use ring::hmac::sign;
use ring::{digest, hmac, rand};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
pub trait Signer {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, Error>;

    fn signature_method(&self) -> SignatureMethod;
}

pub trait Verifier {
    fn verify(&self, msg: &[u8], signature: Vec<u8>) -> Result<bool, Error>;
}

pub struct HmacSha256Signature {
    key: Vec<u8>,
}

impl HmacSha256Signature {
    pub fn new(key: Vec<u8>) -> Self {
        Self { key }
    }
}

impl Signer for HmacSha256Signature {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let s_key = hmac::SigningKey::new(&digest::SHA256, &self.key);
        let signature = hmac::sign(&s_key, msg);
        Ok(Vec::from(signature.as_ref()))
    }

    fn signature_method(&self) -> SignatureMethod {
        SignatureMethod::HmacSha256
    }
}

impl Verifier for HmacSha256Signature {
    fn verify(&self, msg: &[u8], signature: Vec<u8>) -> Result<bool, Error> {
        let v_key = hmac::VerificationKey::new(&digest::SHA256, &self.key);
        Ok(match hmac::verify(&v_key, msg, &signature) {
            Ok(_) => true,
            Err(_) => false,
        })
    }
}

pub fn digest_file<P: AsRef<Path>>(
    file: P,
    algorithm: &'static digest::Algorithm,
) -> Result<digest::Digest, Error> {
    let file = File::open(file)?;
    let mut hash_context = digest::Context::new(algorithm);
    let mut reader = BufReader::new(file);
    let mut buf: Vec<u8> = vec![0; 4096];
    loop {
        let bytes_read = reader.read(buf.as_mut_slice())?;
        if bytes_read == 0 {
            break;
        }
        hash_context.update(&buf[0..bytes_read]);
    }
    Ok(hash_context.finish())
}

pub struct PublishAlgorithms {
    pub signer: Box<Signer>,
    pub checksum_method: ChecksumMethod,
}

impl ChecksumMethod {
    pub fn algorithm(&self) -> &'static digest::Algorithm {
        match self {
            ChecksumMethod::Sha256 => &digest::SHA256,
        }
    }
}

impl Config {
    /// If configured, get the publish algorithms from the publish_parameters
    ///
    /// If not configured or misconfigured (missing key, invalid key, invalid algorithm...)
    /// return a ConfigurationValidationError
    pub fn get_publish_algorithm(&self) -> Result<PublishAlgorithms, ConfigValidationError> {
        match &self.publish_parameters {
            None => Err(ConfigValidationError::NoPublishParameters),
            Some(params) => Ok(PublishAlgorithms {
                checksum_method: params.checksum_method,
                signer: self.get_signer(params)?,
            }),
        }
    }

    pub fn get_signer(
        &self,
        publish_parameters: &PublishParameters,
    ) -> Result<Box<Signer>, ConfigValidationError> {
        // Signer means publish parameters are mandatory:

        match publish_parameters.signature_method {
            SignatureMethod::HmacSha256 => Ok(Box::new(
                self.get_hmac_sha256_signer_verifier(publish_parameters)?,
            )),
        }
    }

    fn get_hmac_sha256_signer_verifier(
        &self,
        publish_parameters: &PublishParameters,
    ) -> Result<HmacSha256Signature, ConfigValidationError> {
        // OMFG this code is shitty but shall be valid since we previously validated publish_parameters

        match &publish_parameters.hmac_sha256_signing_key {
            None => Err(ConfigValidationError::NoHmacKeysConfigured),
            Some(key_id) => Ok(HmacSha256Signature::new(self.get_key_bytes(key_id)?)),
        }
    }
    fn get_key_bytes(&self, key_id: &str) -> Result<Vec<u8>, ConfigValidationError> {
        // get hmac signing keys
        let keys = self
            .hmac_sha256_keys
            .as_ref()
            .ok_or(ConfigValidationError::NoHmacKeysConfigured)?;

        // get the base64 encoded key
        let key = keys
            .get(key_id)
            .ok_or(ConfigValidationError::HmacSigningKeyNotFound {
                key_id: key_id.into(),
            })?;

        // decode key & validate key length
        base64::decode(key)
            .map_err(|e| ConfigValidationError::HmacSigningKeyNotFound {
                key_id: key.clone(),
            })
            .and_then(|key_bytes| {
                // validate key length
                if key_bytes.len() != 32 {
                    Err(ConfigValidationError::InvalidHmac256Key(key.clone()))
                } else {
                    Ok(key_bytes)
                }
            })
    }
}
