use crate::config::Config;
use crate::config::ConfigValidationError;
use crate::config::PublishParameters;
use crate::metadata::{Artifact, ChecksumMethod, SignatureMethod};
use failure::Error;
use ring::hmac::sign;
use ring::{digest, hmac, rand};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

mod hmac_signature;
use hmac_signature::*;

pub trait Signer {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, Error>;

    fn signature_method(&self) -> SignatureMethod;

    fn key_id(&self) -> String;
}

pub trait Verifier {
    fn verify(&self, msg: &[u8], signature: Vec<u8>) -> bool;
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
    pub signer: Box<dyn Signer>,
    pub checksum_method: ChecksumMethod,
}

impl ChecksumMethod {
    pub(crate) fn algorithm(&self) -> &'static digest::Algorithm {
        match self {
            ChecksumMethod::Sha256 => &digest::SHA256,
            ChecksumMethod::Sha384 => &digest::SHA384,
            ChecksumMethod::Sha512 => &digest::SHA512,
        }
    }
}

impl Config {
    /// If configured, get the publish algorithms from the publish_parameters
    ///
    /// If not configured or misconfigured (missing key, invalid key, invalid algorithm...)
    /// return a ConfigurationValidationError
    pub(crate) fn get_publish_algorithm(&self) -> Result<PublishAlgorithms, ConfigValidationError> {
        match &self.publish_parameters {
            None => Err(ConfigValidationError::NoPublishParameters),
            Some(params) => Ok(PublishAlgorithms {
                checksum_method: params.checksum_method,
                signer: self.get_signer(params)?,
            }),
        }
    }

    pub(crate) fn get_verifier(
        &self,
        signature_method: &SignatureMethod,
        key_id: &str,
    ) -> Result<Box<dyn Verifier>, ConfigValidationError> {
        match signature_method {
            SignatureMethod::HmacSha256
            | SignatureMethod::HmacSha384
            | SignatureMethod::HmacSha512 => {
                Ok(Box::new(self.get_hmac_verifier(signature_method, key_id)?))
            }
        }
    }

    pub(crate) fn get_signer(
        &self,
        publish_parameters: &PublishParameters,
    ) -> Result<Box<dyn Signer>, ConfigValidationError> {
        // Signer means publish parameters are mandatory:

        match publish_parameters.signature_method {
            SignatureMethod::HmacSha256
            | SignatureMethod::HmacSha384
            | SignatureMethod::HmacSha512 => {
                Ok(Box::new(self.get_hmac_signer(publish_parameters)?))
            }
        }
    }
}

impl Artifact {
    pub(crate) fn verify_signature(&self, config: &Config) -> Result<bool, Error> {
        let msg: Vec<u8> = self
            .files
            .iter()
            .map(|file| {
                file.name
                    .as_bytes()
                    .iter()
                    .chain(file.checksum.as_bytes().iter())
            })
            .flatten()
            .map(|c| *c)
            .collect();

        let verifier =
            config.get_verifier(&self.signature.signature_method, &self.signature.key_id)?;

        Ok(verifier.verify(&msg, base64::decode(&self.signature.signature)?))
    }
}
