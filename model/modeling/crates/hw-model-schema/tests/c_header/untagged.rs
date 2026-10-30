use super::support::{TestResult, compile_fragments, render_record};
use hw_model_schema::{record_schema, union_schema};

record_schema! { pub record Small { endian: Big, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { prefix: u8, value: u64 } } }
union_schema! { pub union Packet { cases: { Small(Small), Large(Large) } } }

#[test]
fn opaque_extent_and_complete_case_fields_work_in_c11() -> TestResult {
    // Given independent complete record layouts and opaque union metadata.
    let union = render_record("packet.h", "packet", &Packet::SPEC.resolve()?)?;
    let small = render_record("small.h", "small", &Small::SPEC.resolve()?)?;
    let large = render_record("large.h", "large", &Large::SPEC.resolve()?)?;
    // When a real C11 consumer chooses the interpretation explicitly.
    // Then exported extent, offsets and byte-order accessors agree with the wire layout.
    compile_fragments(
        "untagged",
        &[&union.contents, &small.contents, &large.contents],
        br"
int main(void) {
    uint8_t bytes[C_PACKET_SIZE] = {0};
    if (C_PACKET_SIZE != 16 || C_PACKET_ALIGNMENT != 8) return 1;
    if (C_SMALL_C_VALUE_OFFSET != 0 || C_LARGE_C_VALUE_OFFSET != 8) return 2;
    C_SMALL_STORE_C_VALUE(bytes, UINT16_C(0x1234));
    if (bytes[0] != 0x12 || bytes[1] != 0x34) return 3;
    C_LARGE_STORE_C_VALUE(bytes, UINT64_C(0x12345678));
    if (bytes[8] != 0x78 || bytes[11] != 0x12) return 4;
    return 0;
}
",
    )
}
