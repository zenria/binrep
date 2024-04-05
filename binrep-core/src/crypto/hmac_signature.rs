use crate::config::Config;
use crate::config::ConfigValidationError;
use crate::config::PublishParameters;
use crate::metadata::{Artifact, ChecksumMethod, SignatureMethod};

use super::{Signer, Verifier};
use anyhow::Error;
use ring::hkdf::KeyType;
use ring::hmac::Algorithm;
use ring::{digest, hmac};

pub struct HmacShaSignature {
    hmac_signature_method: HmacSignatureMethod,
    signature_method: SignatureMethod,
    key: Vec<u8>,
    key_id: String,
}

#[derive(Copy, Clone)]
struct HmacSignatureMethod(Algorithm);

impl HmacSignatureMethod {
    fn new(signature_method: &SignatureMethod) -> Self {
        match signature_method {
            SignatureMethod::HmacSha256 => Self(hmac::HMAC_SHA256),
            SignatureMethod::HmacSha384 => Self(hmac::HMAC_SHA384),
            SignatureMethod::HmacSha512 => Self(hmac::HMAC_SHA512),
            _ => {
                panic!("You must not call this function for something else than hmac signatures ;)")
            }
        }
    }
    fn digest_algorithm(&self) -> Algorithm {
        self.0
    }

    fn key_len(&self) -> usize {
        self.digest_algorithm().len()
    }
}

impl HmacShaSignature {
    fn new(
        hmac_signature_method: HmacSignatureMethod,
        signature_method: &SignatureMethod,
        key: Vec<u8>,
        key_id: String,
    ) -> Self {
        Self {
            hmac_signature_method,
            signature_method: *signature_method,
            key,
            key_id,
        }
    }
}

impl Signer for HmacShaSignature {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let s_key = hmac::Key::new(self.hmac_signature_method.digest_algorithm(), &self.key);
        let signature = hmac::sign(&s_key, msg);
        Ok(Vec::from(signature.as_ref()))
    }

    fn signature_method(&self) -> SignatureMethod {
        self.signature_method
    }

    fn key_id(&self) -> String {
        self.key_id.clone()
    }
}

impl Verifier for HmacShaSignature {
    fn verify(&self, msg: &[u8], signature: Vec<u8>) -> bool {
        let v_key = hmac::Key::new(self.hmac_signature_method.digest_algorithm(), &self.key);
        match hmac::verify(&v_key, msg, &signature) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

// hmac related config impl
impl Config {
    pub(crate) fn get_hmac_verifier(
        &self,
        signature_method: &SignatureMethod,
        key_id: &str,
    ) -> Result<HmacShaSignature, ConfigValidationError> {
        let hmac_signature_method = HmacSignatureMethod::new(signature_method);
        Ok(HmacShaSignature::new(
            hmac_signature_method,
            signature_method,
            self.get_key_bytes(key_id, &hmac_signature_method)?,
            key_id.to_string(),
        ))
    }

    pub(crate) fn get_hmac_signer(
        &self,
        publish_parameters: &PublishParameters,
    ) -> Result<HmacShaSignature, ConfigValidationError> {
        match &publish_parameters.hmac_signing_key {
            None => Err(ConfigValidationError::NoHmacKeysConfigured),
            Some(key_id) => {
                let hmac_signature_method =
                    HmacSignatureMethod::new(&publish_parameters.signature_method);
                Ok(HmacShaSignature::new(
                    hmac_signature_method,
                    &publish_parameters.signature_method,
                    self.get_key_bytes(key_id, &hmac_signature_method)?,
                    key_id.clone(),
                ))
            }
        }
    }
    fn get_key_bytes(
        &self,
        key_id: &str,
        hmac_signature_method: &HmacSignatureMethod,
    ) -> Result<Vec<u8>, ConfigValidationError> {
        // get hmac signing keys
        let keys = self
            .hmac_keys
            .as_ref()
            .ok_or(ConfigValidationError::NoHmacKeysConfigured)?;

        // get the base64 encoded key
        let key = keys
            .get(key_id)
            .ok_or(ConfigValidationError::HmacSigningKeyNotFound {
                key_id: key_id.into(),
            })?;

        // decode key & validate key length
        data_encoding::BASE64
            .decode(key.as_bytes())
            .map_err(|e| ConfigValidationError::HmacSigningKeyNotFound {
                key_id: key.clone(),
            })
            .and_then(|key_bytes| {
                // validate key length
                if key_bytes.len() != hmac_signature_method.key_len() {
                    Err(ConfigValidationError::InvalidHmacKey(key.clone()))
                } else {
                    Ok(key_bytes)
                }
            })
    }
}
