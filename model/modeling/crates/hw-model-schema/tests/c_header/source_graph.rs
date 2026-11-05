use super::support::{TestResult, compile_fragments, render_record, source};
use hw_model_schema::record_schema;

record_schema! { pub record Leaf { endian: Big, fields: { value: u16 } } }
record_schema! { pub record Pair { endian: Little, fields: { first: Leaf, gap: u32, second: Leaf } } }

#[test]
fn repeated_source_references_preserve_storage_names_and_native_bytes() -> TestResult {
    let actual = source::<Pair>("pair")?;
    let expected = render_record("pair", "pair", &Pair::SPEC)?;
    assert_eq!(actual.contents, expected.contents);
    compile_fragments(
        "source-pair",
        &[&actual.contents],
        br#"
_Static_assert(C_PAIR_C_FIRSTQX2EVALUE_OFFSET == 0, "first occurrence");
_Static_assert(C_PAIR_C_SECONDQX2EVALUE_OFFSET == 8, "second occurrence");
int main(void) {
    uint8_t bytes[C_PAIR_SIZE] = {0};
    C_PAIR_STORE_C_FIRSTQX2EVALUE(bytes, UINT16_C(0x1234));
    C_PAIR_STORE_C_SECONDQX2EVALUE(bytes, UINT16_C(0xabcd));
    return bytes[0] != 0x12 || bytes[1] != 0x34 || bytes[8] != 0xab || bytes[9] != 0xcd;
}
"#,
    )
}

#[test]
fn source_enum_bits_and_tagged_unions_match_existing_c_projection() -> TestResult {
    let inline = source::<super::enum_bit_union::Inline>("inline")?;
    let named = source::<super::enum_bit_union::Named>("named")?;
    let expected_inline = render_record("inline", "inline", &super::enum_bit_union::Inline::SPEC)?;
    let expected_named = render_record("named", "named", &super::enum_bit_union::Named::SPEC)?;
    assert_eq!(inline.contents, expected_inline.contents);
    assert_eq!(named.contents, expected_named.contents);
    compile_fragments(
        "source-tagged",
        &[&inline.contents, &named.contents],
        br"
int main(void) {
    uint8_t a[C_INLINE_SIZE] = {0}; uint8_t b[C_NAMED_SIZE] = {0};
    C_INLINE_STORE_C_WORD0(a, UINT16_C(0x1234));
    C_NAMED_STORE_C_WORD0(b, UINT16_C(0x1234));
    return a[2] != 0x34 || a[3] != 0x12 || b[2] != 0x12 || b[3] != 0x34
        || C_INLINE_C_FLAGS_C_KIND_C_QULARGE != 2 || C_NAMED_C_HEADER_C_KIND_C_QULARGE != 3;
}
",
    )
}

#[test]
fn source_scalar_enums_and_untagged_envelopes_preserve_c_bytes() -> TestResult {
    let packet = source::<super::generated_enumeration::Packet>("packet")?;
    let expected = render_record(
        "packet",
        "packet",
        &super::generated_enumeration::Packet::SPEC,
    )?;
    assert_eq!(packet.contents, expected.contents);
    let union = source::<super::untagged::Packet>("envelope")?;
    let expected = render_record("envelope", "envelope", &super::untagged::Packet::SPEC)?;
    assert_eq!(union.contents, expected.contents);
    compile_fragments(
        "source-enum-envelope",
        &[&packet.contents, &union.contents],
        br#"
_Static_assert(C_ENVELOPE_SIZE == 16, "opaque union extent");
_Static_assert(C_PACKET_C_STATUS_C_QUACTIVE == 5, "enum code");
_Static_assert(C_PACKET_C_FLAGS_C_STATE_C_QUACTIVE == 5, "shared bits code");
int main(void) {
    uint8_t bytes[C_PACKET_SIZE] = {0};
    C_PACKET_STORE_C_STATUS(bytes, C_PACKET_C_STATUS_C_QUACTIVE);
    return bytes[0] != 5;
}
"#,
    )
}

#[test]
fn source_nested_documentation_preserves_existing_c_comments() -> TestResult {
    let actual = source::<super::ordered_descriptions::Packet>("packet")?;
    let expected = render_record(
        "packet",
        "packet",
        &super::ordered_descriptions::Packet::SPEC,
    )?;
    assert_eq!(
        std::str::from_utf8(&actual.contents)?,
        std::str::from_utf8(&expected.contents)?
    );
    compile_fragments(
        "source-documentation",
        &[&actual.contents],
        b"int main(void) { return 0; }\n",
    )
}

record_schema! {
    pub record Envelope {
        endian: Little,
        fields: {
            kind: enum(u8) { variants: { Item = 0 } },
            #[description = "Selected complete record"]
            payload: tagged_union { tag: self.kind, cases: { Item(Leaf) } },
        },
    }
}

#[test]
fn source_tagged_external_case_retains_opaque_placement_metadata() -> TestResult {
    let actual = source::<Envelope>("envelope")?;
    let expected = render_record("envelope", "envelope", &Envelope::SPEC)?;
    assert_eq!(
        std::str::from_utf8(&actual.contents)?,
        std::str::from_utf8(&expected.contents)?
    );
    compile_fragments(
        "source-tagged-envelope",
        &[&actual.contents],
        br#"
_Static_assert(C_ENVELOPE_RECORD_C_PAYLOAD_OFFSET == 2, "envelope offset");
_Static_assert(C_ENVELOPE_RECORD_C_PAYLOAD_SIZE == 2, "envelope extent");
int main(void) { return C_ENVELOPE_SIZE != 4; }
"#,
    )
}
