use hw_model_schema::{
    bits_schema, export::backends::c_header::RecordCHeaderExporter, record_schema,
};

use super::support::{TestResult, compile_fragments, render};

record_schema! {
    pub record Inline {
        endian: Little,
        fields: {
            flags: bits(u16) { fields: {
                ready: 0..=0,
                kind: 3..=4 => enum { variants: { Small = 1, Large = 2 } },
                priority: 5..=7,
            } },
            payload: tagged_union { tag: self.flags.kind,
                cases: { Small { first: u16 }, Large { first: u16, second: u16 } },
            },
        },
    }
}

bits_schema! {
    pub bits Header(u16) {
        ready: 0..=0,
        priority: 1..=3,
        kind: 9..=10 => enum { variants: { Small = 1, Large = 3 } },
    }
}

record_schema! {
    pub record Named {
        endian: Big,
        fields: {
            header: bits(Header),
            payload: tagged_union { tag: self.header.kind,
                cases: { Small { first: u16 }, Large { first: u16, second: u16 } },
            },
        },
    }
}

#[test]
fn bitfield_tag_constants_and_raw_word_accessors_interoperate_with_rust() -> TestResult {
    // Given: distinct packed tag positions, codes and byte orders in two records.
    let inline = Inline::new(
        InlineFlagsValues {
            ready: true,
            kind: InlineFlagsValuesKind::Small,
            priority: 5,
        },
        InlinePayloadValues::Large(InlinePayloadLargeValues {
            first: 0x1234,
            second: 0x5678,
        }),
    );
    let named = Named::new(
        HeaderValues {
            ready: true,
            priority: 5,
            kind: HeaderKind::Small,
        },
        NamedPayloadValues::Large(NamedPayloadLargeValues {
            first: 0x9abc,
            second: 0xdef0,
        }),
    );
    assert_eq!(inline.to_bytes()?, [0xb1, 0, 0x34, 0x12, 0x78, 0x56]);
    assert_eq!(named.to_bytes()?, [6, 0x0b, 0x9a, 0xbc, 0xde, 0xf0]);
    // When: their resolved metadata is exported to real C11 consumers.
    let inline = render(
        &RecordCHeaderExporter::new("inline.h", "inline"),
        &Inline::SPEC.resolve()?,
    )?;
    let named = render(
        &RecordCHeaderExporter::new("named.h", "named"),
        &Named::SPEC.resolve()?,
    )?;
    // Then: unshifted enum codes and word accessors produce the same independent wire fixtures.
    compile_fragments("enum-bit-union", &[&inline.contents, &named.contents], br"
#include <string.h>
int main(void) {
    uint8_t a[C_INLINE_SIZE] = {0};
    uint8_t b[C_NAMED_SIZE] = {0};
    const uint8_t expected_a[] = {0xb1, 0, 0x34, 0x12, 0x78, 0x56};
    const uint8_t expected_b[] = {6, 0x0b, 0x9a, 0xbc, 0xde, 0xf0};
    C_INLINE_STORE_C_FLAGS(a, UINT16_C(1) | (C_INLINE_C_FLAGS_C_KIND_C_QULARGE << 3) | (UINT16_C(5) << 5));
    C_INLINE_STORE_C_WORD0(a, UINT16_C(0x1234));
    C_INLINE_STORE_C_WORD1(a, UINT16_C(0x5678));
    C_NAMED_STORE_C_HEADER(b, UINT16_C(1) | (UINT16_C(5) << 1) | (C_NAMED_C_HEADER_C_KIND_C_QULARGE << 9));
    C_NAMED_STORE_C_WORD0(b, UINT16_C(0x9abc));
    C_NAMED_STORE_C_WORD1(b, UINT16_C(0xdef0));
    if (memcmp(a, expected_a, sizeof(expected_a)) != 0) return 1;
    if (memcmp(b, expected_b, sizeof(expected_b)) != 0) return 2;
    if (((C_INLINE_LOAD_C_FLAGS(a) >> 3) & 3) != C_INLINE_C_FLAGS_C_KIND_C_QULARGE) return 3;
    if (((C_NAMED_LOAD_C_HEADER(b) >> 9) & 3) != C_NAMED_C_HEADER_C_KIND_C_QULARGE) return 4;
    return 0;
}
")?;
    Ok(())
}
