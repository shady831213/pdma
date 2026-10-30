//! Symbol diagnostics retain the first declaration across rejected insertions.
use hw_model_schema::export::SymbolScope;

#[test]
fn collision_preserves_original_declaration_and_reports_both_sources() {
    // Given: a target namespace already contains one declaration.
    let mut scope = SymbolScope::new();
    assert!(scope.insert("VALUE", "first.field").is_ok());
    // When: two later declarations claim the same key.
    let collision = scope.insert("VALUE", "second.field").unwrap_err();
    // Then: each diagnostic points back to the original owner.
    assert_eq!(collision.first, "first.field");
    assert_eq!(collision.second, "second.field");
    let repeated = scope.insert("VALUE", "third.field").unwrap_err();
    assert_eq!(repeated.first, "first.field");
    assert_eq!(repeated.second, "third.field");
    assert!(scope.insert("OTHER", "second.field").is_ok());
}
