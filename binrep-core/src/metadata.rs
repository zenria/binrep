use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Artifacts {
    pub artifacts: Vec<String>,
}
impl Artifacts {
    pub fn new() -> Self {
        Self {
            artifacts: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Versions {
    pub versions: Vec<Version>,
}

impl Versions {
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Copy)]
pub enum ChecksumMethod {
    #[serde(rename = "SHA256")]
    Sha256,
    #[serde(rename = "SHA384")]
    Sha384,
    #[serde(rename = "SHA512")]
    Sha512,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct File {
    pub name: String,
    pub checksum: String,
    pub checksum_method: ChecksumMethod,
    pub unix_mode: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Copy)]
pub enum SignatureMethod {
    #[serde(rename = "HMAC_SHA256")]
    HmacSha256,
    #[serde(rename = "HMAC_SHA384")]
    HmacSha384,
    #[serde(rename = "HMAC_SHA512")]
    HmacSha512,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Signature {
    pub key_id: String,
    pub signature: String,
    pub signature_method: SignatureMethod,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Artifact {
    pub version: Version,
    pub signature: Signature,
    pub files: Vec<File>,
}

impl fmt::Display for Artifact {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} - {}", self.version, self.signature.signature)?;
        for file in &self.files {
            write!(f, "\n  {} - {}", file.name, file.checksum)?;
            if let Some(unix_mode) = file.unix_mode {
                write!(f, " - {:o}", unix_mode)?;
            }
        }
        Ok(())
    }
}
