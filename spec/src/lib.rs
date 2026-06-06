// prim-spec: blackbox acceptance scenarios for the `prim` binary.
//
// This crate is test-only (`publish = false`). Stateless CLI output
// snapshots use `trycmd` (tests/cmd/**); behavioural scenarios that touch
// the filesystem or stdin use `assert_cmd` against real temp files.
