use crate::Version;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Artifacts {
    pub artifacts: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Latest {
    pub latest_version: Version,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Versions {
    pub versions: Vec<Version>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
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
    key_id: String,
    signature: String,
    signature_method: SignatureMethod,
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Artifact {
    pub version: Version,
    pub files: Vec<File>,
    pub signature_method: SignatureMethod,
}
