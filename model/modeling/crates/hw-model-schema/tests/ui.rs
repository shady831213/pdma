//! Compile-pass and compile-fail schema macro tests.

#[test]
fn schema_macro_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}

#[test]
fn map_values_decode_from_is_private() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/fail/map_values_decode_from_is_private.rs");
}

#[test]
fn access_path_pass() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/access_path_pass.rs");
}

#[test]
fn access_path_fail() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/fail/access_path_*.rs");
}
