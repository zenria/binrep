use binrep_core::binrep::Binrep;
use binrep_core::progress::NOOPProgress;

#[test]
fn test_with_current_config() {
    let _binrep = Binrep::<NOOPProgress>::new::<String>(&None).unwrap();
}
