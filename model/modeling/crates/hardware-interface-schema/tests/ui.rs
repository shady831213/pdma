//! Compile-pass and compile-fail schema macro tests.

#[test]
fn schema_macro_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}
