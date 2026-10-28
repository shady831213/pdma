use super::{ModernRecordDecl, UntaggedUnionDecl};

#[test]
fn tagged_union_keyword_accepts_a_tagged_field() {
    // Given: an inline union with an explicitly selected discriminator.
    let source = "record Packet { endian: little, fields: {
        kind: enum(u8) { variants: { A = 1 } },
        body: tagged_union { tag: self.kind, cases: { A { value: u32 } } }
    } }";
    // When: the record crosses the syntax boundary.
    let parsed = syn::parse_str::<ModernRecordDecl>(source);
    // Then: the explicit tagged syntax is accepted.
    assert!(parsed.is_ok(), "{}", parsed.err().unwrap());
}

#[test]
fn old_inline_union_keyword_requires_migration() {
    // Given: the former spelling of a tagged field.
    let source = "record Packet { endian: little, fields: {
        body: union { tag: self.kind, cases: { A { value: u32 } } }
    } }";
    // When: the record crosses the syntax boundary.
    let error = syn::parse_str::<ModernRecordDecl>(source).err();
    // Then: it is rejected with the replacement spelling.
    assert!(error.is_some_and(|error| error.to_string().contains("use `tagged_union`")));
}

#[test]
fn tagged_union_requires_a_tag() {
    // Given: explicit tagged syntax without its discriminator.
    let source = "record Packet { endian: little, fields: {
        body: tagged_union { cases: { A { value: u32 } } }
    } }";
    // When: the record crosses the syntax boundary.
    let error = syn::parse_str::<ModernRecordDecl>(source).err();
    // Then: rejection identifies the missing discriminator.
    assert!(error.is_some_and(|error| error.to_string().contains("expected `tag`")));
}

#[test]
fn untagged_union_rejects_a_tag() {
    // Given: standalone storage with an invalid discriminator clause.
    let source = "pub union Packet { tag: self.kind, cases: { A(Record) } }";
    // When: the storage declaration crosses the syntax boundary.
    let error = syn::parse_str::<UntaggedUnionDecl>(source).err();
    // Then: it is rejected as a tagged/untagged category error.
    assert!(error.is_some_and(|error| {
        error
            .to_string()
            .contains("untagged union cannot declare `tag`")
    }));
}

#[test]
fn tagged_union_rejects_trailing_clauses() {
    // Given: a complete tagged field followed by an extra clause.
    for clause in ["tag: self.other", "garbage: 7"] {
        let source = format!(
            "record Packet {{ endian: Little, fields: {{
            body: tagged_union {{ tag: self.kind, cases: {{ A {{ value: u32 }} }}, {clause} }}
        }} }}"
        );
        // When: the record crosses the syntax boundary.
        let parsed = syn::parse_str::<ModernRecordDecl>(&source);
        // Then: neither duplicate discriminators nor unknown clauses are ignored.
        assert!(parsed.is_err(), "accepted trailing clause {clause}");
    }
}

#[test]
fn untagged_union_accepts_only_optional_external_codec() {
    // Given the sole optional adapter clause, with either trailing-comma spelling.
    for suffix in [", codec: external", ", codec: external,"] {
        let source = format!("pub union Packet {{ cases: {{ A(Record) }}{suffix} }}");
        // When parsing the explicit opt-in.
        let parsed = syn::parse_str::<UntaggedUnionDecl>(&source);
        // Then no selector or callback declaration is required.
        assert!(parsed.is_ok(), "{}", parsed.err().unwrap());
    }
}

#[test]
fn untagged_union_rejects_malformed_codec_options() {
    // Given unsupported, duplicated, or misplaced codec options.
    for body in [
        "cases: { A(Record) }, codec: inferred",
        "cases: { A(Record) }, codec: external, codec: external",
        "cases: { A(Record) }, codec:",
        "cases: { A(Record) } codec: external",
        "codec: external, cases: { A(Record) }",
    ] {
        // When parsing each malformed declaration.
        let parsed = syn::parse_str::<UntaggedUnionDecl>(&format!("union Packet {{ {body} }}"));
        // Then none silently changes the union contract.
        assert!(parsed.is_err(), "accepted {body}");
    }
}
