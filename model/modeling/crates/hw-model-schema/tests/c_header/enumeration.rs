use hw_model_schema::{
    BitsFieldDefinition, BitsFieldSpec, Documentation, Endian, EnumDefinition, EnumVariantSpec,
    FieldSpec, RecordPlacement, RecordSpec, ScalarKind,
    register::{RegisterDefinition, RegisterSpec},
    register_map::{RegisterMapSpec, RepeatedRegisterBank},
};

use super::support::{TestResult, compile_fragments, render_map, render_record};

const VARIANTS: [EnumVariantSpec; 3] = [
    EnumVariantSpec::new("idle", 0),
    EnumVariantSpec::new("active-state", 7).with_documentation(Documentation {
        short_description: Some("Active /* transfer */"),
        description: Some("*/\n#error injected\n/*\n*\\\n/\n*??/\n/"),
    }),
    EnumVariantSpec::new("maximum", u64::MAX),
];
const FIELDS: [FieldSpec; 1] =
    [FieldSpec::new(0, "state", 0, ScalarKind::U64).with_enum(EnumDefinition::new(&VARIANTS))];
const CHILD: RecordSpec = RecordSpec::named("Child", 8, 8, &FIELDS, &[]);
const CHILDREN: [RecordPlacement; 1] = [RecordPlacement::new(0, "child", 0, &CHILD)];
const RECORD: RecordSpec = RecordSpec::named("Record", 8, 8, &[], &CHILDREN);
const BIT_VARIANTS: [EnumVariantSpec; 2] = [VARIANTS[0], VARIANTS[1]];
const BITS: [BitsFieldSpec; 1] = [BitsFieldSpec::place(
    0,
    "mode",
    4,
    BitsFieldDefinition::new(3).with_enum(EnumDefinition::new(&BIT_VARIANTS)),
)];
const BIT_FIELDS: [FieldSpec; 1] = [FieldSpec::new(0, "flags", 0, ScalarKind::U8).with_bits(&BITS)];
const BIT_CHILD: RecordSpec = RecordSpec::named("Child", 1, 1, &BIT_FIELDS, &[]);
const BIT_CHILDREN: [RecordPlacement; 1] = [RecordPlacement::new(0, "child", 0, &BIT_CHILD)];
const BIT_RECORD: RecordSpec = RecordSpec::named("Record", 1, 1, &[], &BIT_CHILDREN);
const REGISTER: RegisterDefinition =
    RegisterDefinition::new("Status", 4, Endian::Little).with_fields(&BITS);
const REGISTERS: [RegisterSpec<u16>; 1] = [RegisterSpec::place(0, "status", 0, REGISTER)];
const BANK_REGISTERS: [RegisterSpec<u16>; 1] = [RegisterSpec::place(1, "status", 0, REGISTER)];
const BANKS: [RepeatedRegisterBank<u16>; 1] = [RepeatedRegisterBank::new(
    "channels",
    4,
    2,
    4,
    &BANK_REGISTERS,
)];
const MAP: RegisterMapSpec<u16> = RegisterMapSpec::new(12, &REGISTERS, &BANKS);

#[test]
fn record_enum_codes_and_sanitized_documentation_compile_in_c11() -> TestResult {
    // Given: nested enum metadata with sparse codes and hostile comment delimiters.
    let exporter = ("enum.h", "record");
    // When: the resolved record exports its enum variants.
    let artifact = render_record(exporter.0, exporter.1, &RECORD)?;
    // Then: variant descriptions remain comments and every wire code is C-consumable.
    let text = std::str::from_utf8(&artifact.bytes)?;
    assert!(text.contains("Active / * transfer * /"));
    compile_fragments(
        "record-enum",
        &[&artifact.bytes],
        br#"
_Static_assert(C_RECORD_C_CHILDQX2ESTATE_C_IDLE == 0, "idle code");
_Static_assert(C_RECORD_C_CHILDQX2ESTATE_C_ACTIVEQX2DSTATE == 7, "active code");
_Static_assert(C_RECORD_C_CHILDQX2ESTATE_C_MAXIMUM == UINT64_MAX, "full width code");
int main(void) {
    uint8_t bytes[C_RECORD_SIZE] = {0};
    C_RECORD_STORE_C_CHILDQX2ESTATE(bytes, C_RECORD_C_CHILDQX2ESTATE_C_ACTIVEQX2DSTATE);
    return C_RECORD_LOAD_C_CHILDQX2ESTATE(bytes) != UINT64_C(7);
}
"#,
    )?;
    Ok(())
}

#[test]
fn nested_record_bit_enum_exports_unshifted_codes() -> TestResult {
    // Given: an enum embedded at bit four within a nested record field.
    let exporter = ("bits.h", "record");
    // When: the nested bit metadata crosses the record exporter.
    let artifact = render_record(exporter.0, exporter.1, &BIT_RECORD)?;
    // Then: named codes remain local values and hostile docs remain inert comments.
    assert!(std::str::from_utf8(&artifact.bytes)?.contains("Active / * transfer * /"));
    compile_fragments(
        "record-bit-enum",
        &[&artifact.bytes],
        br#"
_Static_assert(C_RECORD_C_CHILDQX2EFLAGS_C_MODE_C_IDLE == 0, "idle code");
_Static_assert(C_RECORD_C_CHILDQX2EFLAGS_C_MODE_C_ACTIVEQX2DSTATE == 7, "local code");
int main(void) { return 0; }
"#,
    )?;
    Ok(())
}

#[test]
fn fixed_and_repeated_register_bit_enums_export_the_same_codes() -> TestResult {
    // Given: fixed and repeated registers sharing an enum bit definition.
    let exporter = ("map.h", "map");
    // When: the complete register map exports its field metadata.
    let artifact = render_map(exporter.0, exporter.1, &MAP.resolve()?)?;
    // Then: both register forms expose unshifted codes and sanitized variant docs.
    assert_eq!(
        std::str::from_utf8(&artifact.bytes)?
            .matches("Active / * transfer * /")
            .count(),
        2
    );
    compile_fragments(
        "register-bit-enum",
        &[&artifact.bytes],
        br#"
_Static_assert(C_MAP_C_STATUS_C_MODE_C_IDLE == 0, "fixed idle code");
_Static_assert(C_MAP_C_STATUS_C_MODE_C_ACTIVEQX2DSTATE == 7, "fixed local code");
_Static_assert(C_MAP_C_CHANNELS_C_STATUS_C_MODE_C_ACTIVEQX2DSTATE == 7, "bank local code");
int main(void) { return 0; }
"#,
    )?;
    Ok(())
}
