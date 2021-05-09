use binrep_core::binrep::Binrep;
use binrep_core::config::{Config, ED25519Key};
use binrep_core::progress::NOOPProgress;
use semver::Version;
use std::collections::HashMap;

#[test]
pub fn parse_config() {
    // signing aware config
    let c = Config::read_from_file("tests/config_ed25519_sign.sane").unwrap();
    let k = c.ed25519_keys.unwrap();
    let k = k
        .get(&c.publish_parameters.unwrap().ed25519_signing_key.unwrap())
        .unwrap();
    assert_eq!(*k, ED25519Key::SignAndVerify{
        pkcs8:    "MFMCAQEwBQYDK2VwBCIEIIs/h3QgK0hSPeYJqvNoXARyCgjuLTwMVOPdtlK3HYXBoSMDIQD5s1MF9Sw8VK4vxtF9/bQ+AwJjMFMY5xQsc9qJ4ULm3A==".to_string()
    });

    // verify only aware config
    let c = Config::read_from_file("tests/config_ed25519_verify.sane").unwrap();
    let k = c.ed25519_keys.unwrap();
    let k = k.get("test-key").unwrap();
    assert_eq!(
        *k,
        ED25519Key::Verify {
            public_key: "+bNTBfUsPFSuL8bRff20PgMCYzBTGOcULHPaieFC5tw=".to_string()
        }
    );
}

#[tokio::test]
pub async fn full_test() {
    // Test uploading & decoding with the pkcs8 encoded ed25519 key
    let config = Config::create_file_test_config_ed25519_publish();
    let publish_config = config.clone();
    let mut binrep = Binrep::<NOOPProgress>::from_config(config).unwrap();
    let v1 = Version::new(1, 0, 0);
    let a = binrep.push("cargo", &v1, &["Cargo.toml"]).await.unwrap();
    println!("Pushed {:#?}", a);
    let tmp = tempfile::tempdir().unwrap();
    binrep.pull("cargo", &v1, &tmp, true).await.unwrap();

    // derive the above config as if we only have a ed25519 public key
    let mut config = publish_config.clone();
    config.publish_parameters = None;
    let mut ed25519_keys = HashMap::new();
    ed25519_keys.insert(
        "test".to_string(),
        ED25519Key::Verify {
            public_key: "+bNTBfUsPFSuL8bRff20PgMCYzBTGOcULHPaieFC5tw=".to_string(),
        },
    );
    config.ed25519_keys = Some(ed25519_keys);
    let mut binrep = Binrep::<NOOPProgress>::from_config(config).unwrap(); // new binrep instance
    let tmp = tempfile::tempdir().unwrap(); // new tmp dir
    binrep.pull("cargo", &v1, &tmp, true).await.unwrap();
}
