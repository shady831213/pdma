//! Compile-time renderer capability boundaries.
#[test]
fn renderer_capabilities_do_not_expose_raw_trees_or_dispatch_overrides() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/render_ui/*.rs");
}
