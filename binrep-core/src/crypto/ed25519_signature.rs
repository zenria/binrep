use crate::config::{Config, ConfigValidationError, ED25519Key, PublishParameters};
use crate::crypto::{Signer, Verifier};
use crate::metadata::SignatureMethod;
use anyhow::Error;
use ring::signature;
use ring::signature::{KeyPair, UnparsedPublicKey};
use std::collections::hash_map::RandomState;
use std::collections::HashMap;

pub struct ED25519Signer {
    private_key: Vec<u8>,
    key_id: String,
}

impl Signer for ED25519Signer {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let key = signature::Ed25519KeyPair::from_pkcs8(&self.private_key)?;
        Ok(Vec::from(key.sign(msg).as_ref()))
    }

    fn signature_method(&self) -> SignatureMethod {
        SignatureMethod::ED25519
    }

    fn key_id(&self) -> String {
        self.key_id.clone()
    }
}

pub struct ED25519Verifier {
    public_key: Vec<u8>,
}

impl Verifier for ED25519Verifier {
    fn verify(&self, msg: &[u8], signature: Vec<u8>) -> bool {
        signature::UnparsedPublicKey::new(&signature::ED25519, &self.public_key)
            .verify(msg, &signature)
            .is_ok()
    }
}

impl Config {
    pub(crate) fn get_ed25519_signer(
        &self,
        publish_parameters: &PublishParameters,
    ) -> Result<ED25519Signer, ConfigValidationError> {
        let key_id = publish_parameters
            .ed25519_signing_key
            .as_ref()
            .ok_or(ConfigValidationError::NoED25519SigningKeyConfigured)?;
        Ok(ED25519Signer {
            private_key: self.get_ed25519_key(key_id)?.get_private_key()?,
            key_id: key_id.to_string(),
        })
    }

    pub(crate) fn get_ed25519_verifier(
        &self,
        key_id: &str,
    ) -> Result<ED25519Verifier, ConfigValidationError> {
        Ok(ED25519Verifier {
            public_key: self.get_ed25519_key(key_id)?.get_public_key()?,
        })
    }

    fn get_ed25519_key(&self, key_id: &str) -> Result<&ED25519Key, ConfigValidationError> {
        let keys = self
            .ed25519_keys
            .as_ref()
            .ok_or(ConfigValidationError::NoED25519KeysConfigured)?;
        keys.get(key_id)
            .ok_or(ConfigValidationError::ED25519SigningKeyNotFound {
                key_id: key_id.to_string(),
            })
    }
}

impl ED25519Key {
    fn get_public_key(&self) -> Result<Vec<u8>, ConfigValidationError> {
        match self {
            ED25519Key::SignAndVerify { pkcs8 } => {
                let key_pair =
                    signature::Ed25519KeyPair::from_pkcs8(&base64::decode(pkcs8).map_err(|e| {
                        ConfigValidationError::MalformedED25519Key {
                            cause: e.to_string(),
                        }
                    })?)
                    .map_err(|key_rejected| {
                        ConfigValidationError::MalformedED25519Key {
                            cause: key_rejected.to_string(),
                        }
                    })?;
                Ok(Vec::from(key_pair.public_key().as_ref()))
            }
            ED25519Key::Verify { public_key } => {
                base64::decode(public_key).map_err(|e| ConfigValidationError::MalformedED25519Key {
                    cause: e.to_string(),
                })
            }
        }
    }

    fn get_private_key(&self) -> Result<Vec<u8>, ConfigValidationError> {
        match self {
            ED25519Key::SignAndVerify { pkcs8 } => {
                base64::decode(pkcs8).map_err(|e| ConfigValidationError::MalformedED25519Key {
                    cause: e.to_string(),
                })
            }
            ED25519Key::Verify { .. } => Err(ConfigValidationError::MalformedED25519Key {
                cause: "PKCS8 key data is needed for signing".to_string(),
            }),
        }
    }
}
