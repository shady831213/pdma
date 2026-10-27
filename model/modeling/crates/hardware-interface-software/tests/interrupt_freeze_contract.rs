//! Compile-time surface pin for recoverable registry freezing.

use hardware_interface_software::HandlerRegistry;

#[test]
fn empty_freeze_returns_the_original_builder() {
    let registry = HandlerRegistry::<u8, (), ()>::new();
    let error = registry.freeze().expect_err("empty registry");
    let recovered: HandlerRegistry<u8, (), ()> = error.into_registry();
    assert!(recovered.is_empty());
}
