use super::{BitsSchemaDecl, ModernRecordDecl, OrderedMapDecl, RegisterDefinitionDecl};

#[test]
fn description_attributes_are_accepted_on_declarations_and_members() {
    // Given declarations containing independent and combined documentation attributes.
    let bits = r##"#[description = r#"Shared
bits"#] pub bits Flags(u8) { #[short_description = "Enabled"] enabled: 0..1 }"##;
    let register = r#"#[short_description = "Control"] register Control {
        width: 1, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1,
        fields: { #[description = "Enable copying."] enabled: 0..1 }
    }"#;
    let record = r#"#[description = "Packet"] record Packet { endian: Little, fields: {
        #[short_description = "Payload"] child: Child,
        #[description = "State"] state: bits(u8) { fields: { #[description = "Ready"] ready: 0..1 } }
    } }"#;
    let map = r#"#[short_description = "Device"] map Device { alignment: 1, placements: [
        #[description = "Channel control"] control: Control
    ] }"#;
    // When each public grammar parses the source.
    let results = [
        syn::parse_str::<BitsSchemaDecl>(bits).map(|_| ()),
        syn::parse_str::<RegisterDefinitionDecl>(register).map(|_| ()),
        syn::parse_str::<ModernRecordDecl>(record).map(|_| ()),
        syn::parse_str::<OrderedMapDecl>(map).map(|_| ()),
    ];
    // Then every grammar accepts descriptions at both levels.
    for result in results {
        assert!(result.is_ok(), "{}", result.err().unwrap());
    }
}

#[test]
fn invalid_description_attributes_report_specific_errors() {
    // Given invalid attributes at the declaration boundary.
    let cases = [
        (
            r#"#[short_description = "a"] #[short_description = "b"]"#,
            "duplicate documentation attribute",
        ),
        (
            r#"#[description = "a"] #[description = "b"]"#,
            "duplicate documentation attribute",
        ),
        (
            "#[description = 42]",
            "documentation attribute must be a string literal",
        ),
        (
            "#[description = concat!(\"a\")]",
            "documentation attribute must be a string literal",
        ),
        ("#[other = \"a\"]", "unsupported schema attribute"),
    ];
    for (attributes, expected) in cases {
        // When a real bits declaration is parsed.
        let result = syn::parse_str::<BitsSchemaDecl>(&format!(
            "{attributes} bits Flags(u8) {{ enabled: 0..1 }}"
        ));
        // Then invalid metadata is rejected with its applicable diagnostic.
        let error = result.err().expect("invalid attribute");
        assert!(error.to_string().contains(expected), "{error}");
    }
}
