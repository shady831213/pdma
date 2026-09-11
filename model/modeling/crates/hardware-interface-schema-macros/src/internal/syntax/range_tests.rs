use super::{BitFieldDecl, BitsSchemaDecl};
use crate::internal::validate::bits;

#[test]
fn half_open_ranges_keep_their_existing_endpoints() -> syn::Result<()> {
    // Given
    for (range, start, end) in [("0..1", 0, 1), ("7..10", 7, 10), ("0..64", 0, 64)] {
        // When
        let field = syn::parse_str::<BitFieldDecl>(&format!("value: {range}"))?;
        // Then
        assert_eq!((field.start.0, field.end.0), (start, end));
    }
    Ok(())
}

#[test]
fn inclusive_ranges_normalize_before_width_validation() -> syn::Result<()> {
    // Given
    for (range, start, end) in [
        ("0..=0", 0, 1),
        ("7..=9", 7, 10),
        ("63..=63", 63, 64),
        ("0..=63", 0, 64),
    ] {
        // When
        let field = syn::parse_str::<BitFieldDecl>(&format!("value: {range}"))?;
        // Then
        assert_eq!((field.start.0, field.end.0), (start, end));
        bits::check(&[field], 64)?;
    }
    Ok(())
}

#[test]
fn inclusive_endpoint_overflow_is_a_diagnostic_not_a_panic() {
    // Given
    let source = format!("value: 0..={}", usize::MAX);
    // When
    let result = syn::parse_str::<BitFieldDecl>(&source);
    // Then
    assert_eq!(
        result.err().expect("overflow must fail").to_string(),
        "inclusive bit range endpoint overflows usize"
    );
}

#[test]
fn invalid_ranges_still_fail_shared_validation() {
    for fields in [
        "a: 5..=3",
        "a: 8..=8",
        "a: 0..=8",
        "a: 0..=3, b: 3..=7",
        "a: 0..4, b: 3..=7",
    ] {
        // Given
        let source = format!("bits Invalid(u8) {{ {fields} }}");
        // When
        let declaration = syn::parse_str::<BitsSchemaDecl>(&source).expect("valid range syntax");
        let result = bits::check_schema(&declaration.fields, 8);
        // Then
        assert!(result.is_err(), "accepted {source}");
    }
}

#[test]
fn inclusive_enum_uses_the_complete_declared_width() -> syn::Result<()> {
    // Given: maximum legal discriminant in a three-bit field.
    let source = "bits Codes(u16) { code: 7..=9 => enum { variants: { Max = 7 } } }";
    // When
    let declaration = syn::parse_str::<BitsSchemaDecl>(source)?;
    // Then
    bits::check_schema(&declaration.fields, 16)?;
    let overflow = syn::parse_str::<BitsSchemaDecl>(&source.replace("Max = 7", "Max = 8"))?;
    assert!(bits::check_schema(&overflow.fields, 16).is_err());
    Ok(())
}
