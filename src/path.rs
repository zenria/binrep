#[inline(always)]
pub fn artifacts() -> &'static str {
    "/artifacts.sane"
}
pub mod artifact {
    use semver::Version;
    #[inline(always)]
    pub fn versions(artifact_name: &str) -> String {
        vec![artifact_name, "/versions.sane"].into_iter().collect()
    }
    #[inline(always)]
    pub fn artifact(artifact_name: &str, artifact_version: &Version) -> String {
        vec![
            artifact_name,
            "/",
            &format!("{}", artifact_version),
            "/artifact.sane",
        ]
        .into_iter()
        .collect()
    }
    #[inline(always)]
    pub fn artifact_file(
        artifact_name: &str,
        artifact_version: &Version,
        filename: &str,
    ) -> String {
        vec![
            artifact_name,
            "/",
            &format!("{}", artifact_version),
            "/",
            filename,
        ]
        .into_iter()
        .collect()
    }
}
