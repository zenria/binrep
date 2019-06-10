use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use std::convert::TryFrom;

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
pub struct Latest {
    pub latest_version: Version,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct File {
    pub name: String,
    pub checksum: String,
    pub checksum_method: ChecksumMethod,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum SignatureMethod {
    #[serde(rename = "HMAC_SHA256")]
    HmacSha256,
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
