use binrep_core::binrep::Binrep;

#[test]
fn test_with_current_config() {
    let _binrep = Binrep::new::<String>(&None).unwrap();
}
