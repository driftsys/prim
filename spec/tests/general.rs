// Stateless CLI-output snapshots, driven by trycmd cases under tests/cmd/.
#[test]
fn trycmd_general() {
    trycmd::TestCases::new().case("tests/cmd/general/*.toml");
}
