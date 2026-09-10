use hardware_interface_schema::{
    bits_schema,
    export::backends::c_header::{RecordCHeaderExporter, RegisterMapCHeaderExporter},
    record_schema,
    register::RegisterSpec,
    register_map::RegisterMapSpec,
    register_schema,
};

use super::support::{TestResult, compile_fragments, render};

bits_schema! {
    pub bits Codes(u8) {
        state: 4..7 => enum { variants: {
            Idle = 0,
            #[short_description = "Active /* state */"]
            #[description = "*/\n#error injected\n/*"]
            Active = 5,
        } },
    }
}

record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            status: enum(u8) { variants: { Idle = 0, Active = 5 } },
            flags: bits(Codes),
        },
    }
}

register_schema! {
    pub register Status {
        width: 4, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0,
        fields: Codes,
    }
}

const REGISTERS: [RegisterSpec<u16>; 1] = [RegisterSpec::place(0, "status", 0, Status::SPEC)];
const MAP: RegisterMapSpec<u16> = RegisterMapSpec::new(4, &REGISTERS, &[]);

#[test]
fn generated_enums_reach_real_c_consumers_from_each_embedding() -> TestResult {
    // Given: macro-generated scalar and shared-bit enums in records and registers.
    let record = Packet::SPEC.resolve()?;
    let map = MAP.resolve()?;
    // When: both resolved schemas cross their public C exporters.
    let record_artifact = render(&RecordCHeaderExporter::new("packet.h", "packet"), &record)?;
    let map_artifact = render(&RegisterMapCHeaderExporter::new("map.h", "map"), &map)?;
    // Then: generated metadata retains the same numeric codes and safe descriptions.
    for artifact in [&record_artifact, &map_artifact] {
        assert!(std::str::from_utf8(&artifact.contents)?.contains("Active / * state * /"));
    }
    compile_fragments(
        "generated-enum",
        &[&record_artifact.contents, &map_artifact.contents],
        br#"
_Static_assert(C_PACKET_C_STATUS_C_QUACTIVE == 5, "scalar enum");
_Static_assert(C_PACKET_C_FLAGS_C_STATE_C_QUACTIVE == 5, "record bits enum");
_Static_assert(C_MAP_C_STATUS_C_STATE_C_QUACTIVE == 5, "register bits enum");
int main(void) {
    uint8_t bytes[C_PACKET_SIZE] = {0};
    C_PACKET_STORE_C_STATUS(bytes, C_PACKET_C_STATUS_C_QUACTIVE);
    return C_PACKET_LOAD_C_STATUS(bytes) != UINT64_C(5);
}
"#,
    )?;
    Ok(())
}
