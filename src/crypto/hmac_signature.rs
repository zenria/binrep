use crate::config::Config;
use crate::config::ConfigValidationError;
use crate::config::PublishParameters;
use crate::metadata::{Artifact, ChecksumMethod, SignatureMethod};

use super::{Signer, Verifier};
use failure::Error;
use ring::digest::Algorithm;
use ring::{digest, hmac};

pub struct HmacShaSignature {
    hmac_signature_method: HmacSignatureMethod,
    signature_method: SignatureMethod,
    key: Vec<u8>,
    key_id: String,
}

#[derive(Copy, Clone)]
struct HmacSignatureMethod(&'static Algorithm);

impl HmacSignatureMethod {
    fn new(signature_method: &SignatureMethod) -> Self {
        match signature_method {
            SignatureMethod::HmacSha256 => Self(&digest::SHA256),
            SignatureMethod::HmacSha384 => Self(&digest::SHA384),
            SignatureMethod::HmacSha512 => Self(&digest::SHA512),
        }
    }
    fn digest_algorithm(&self) -> &'static Algorithm {
        self.0
    }

    fn key_len(&self) -> usize {
        self.digest_algorithm().output_len
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
        let s_key = hmac::SigningKey::new(&digest::SHA256, &self.key);
        let signature = hmac::sign(&s_key, msg);
        Ok(Vec::from(signature.as_ref()))
    }

    fn signature_method(&self) -> SignatureMethod {
        SignatureMethod::HmacSha256
    }

    fn key_id(&self) -> String {
        self.key_id.clone()
    }
}

impl Verifier for HmacShaSignature {
    fn verify(&self, msg: &[u8], signature: Vec<u8>) -> bool {
        let v_key = hmac::VerificationKey::new(&digest::SHA256, &self.key);
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
        base64::decode(key)
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
