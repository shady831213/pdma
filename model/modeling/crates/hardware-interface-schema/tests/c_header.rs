//! Generic C11 fragment exporter integration tests.

#![cfg(feature = "export-c-header")]

#[path = "c_header/support.rs"]
#[allow(missing_docs)]
pub mod support;

use support::{
    COLLISION_C_MAIN, HAPPY_C_MAIN, PREFIX_C_MAIN, TestResult, compile_fragments, render,
};

use hardware_interface_schema::{
    Endian, FieldSpec, RecordSpec, ScalarKind,
    export::backends::c_header::{RecordCHeaderExporter, RegisterMapCHeaderExporter},
    register::{Access, RegisterSpec, ResetKind},
    register_map::{RegisterMapSpec, RepeatedRegisterBank},
};

const RECORD_FIELDS: [FieldSpec; 4] = [
    FieldSpec::new(1, "tag", 0, ScalarKind::U8),
    FieldSpec::new(2, "sequence", 2, ScalarKind::U16).with_endian(Endian::Big),
    FieldSpec::new(3, "measure", 5, ScalarKind::U32),
    FieldSpec::new(4, "timestamp", 10, ScalarKind::U64).with_endian(Endian::Big),
];
const RECORD: RecordSpec = RecordSpec::new(18, 2, &RECORD_FIELDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisterId {
    Status,
    Command,
    Level,
    Flags,
}

const FIXED_REGISTERS: [RegisterSpec<RegisterId>; 2] = [
    RegisterSpec::new(RegisterId::Status, "status", 0, 4)
        .with_reset(ResetKind::Constant, 0x1122_3344),
    RegisterSpec::new(RegisterId::Command, "command", 8, 2)
        .with_access(Access::WriteOnly)
        .with_reset(ResetKind::PostReset, 0)
        .with_writable_mask(0xffff),
];
const BANK_REGISTERS: [RegisterSpec<RegisterId>; 2] = [
    RegisterSpec::new(RegisterId::Level, "level", 0, 8)
        .with_access(Access::ReadWrite)
        .with_reset(ResetKind::Constant, 0x0102_0304_0506_0708)
        .with_writable_mask(0x00ff_00ff_00ff_00ff),
    RegisterSpec::new(RegisterId::Flags, "flags", 0x10, 1),
];
const BANKS: [RepeatedRegisterBank<RegisterId>; 1] = [RepeatedRegisterBank::new(
    "channels",
    0x20,
    2,
    0x20,
    &BANK_REGISTERS,
)];
const MAP: RegisterMapSpec<RegisterId> = RegisterMapSpec::new(0x80, &FIXED_REGISTERS, &BANKS);
const REGISTER_GROUPS: [&[RegisterSpec<RegisterId>]; 1] = [&FIXED_REGISTERS];
const GROUPED_MAP: RegisterMapSpec<RegisterId> =
    RegisterMapSpec::new(0x80, &[], &[]).with_register_groups(&REGISTER_GROUPS);

const COLLISION_FIELDS: [FieldSpec; 2] = [
    FieldSpec::new(1, "a-b", 0, ScalarKind::U8),
    FieldSpec::new(2, "a_b", 1, ScalarKind::U16),
];
const COLLISION_RECORD: RecordSpec = RecordSpec::new(3, 1, &COLLISION_FIELDS);
const COLLISION_FIXED: [RegisterSpec<u8>; 2] = [
    RegisterSpec::new(1, "a-b", 0, 1),
    RegisterSpec::new(2, "a_b", 4, 1),
];
const COLLISION_MEMBERS: [RegisterSpec<u8>; 2] = [
    RegisterSpec::new(3, "a-b", 0, 1),
    RegisterSpec::new(4, "a_b", 1, 1),
];
const COLLISION_BANKS: [RepeatedRegisterBank<u8>; 2] = [
    RepeatedRegisterBank::new("bank-a", 0x20, 1, 4, &COLLISION_MEMBERS),
    RepeatedRegisterBank::new("bank_a", 0x40, 1, 4, &COLLISION_MEMBERS),
];
const COLLISION_MAP: RegisterMapSpec<u8> =
    RegisterMapSpec::new(0x80, &COLLISION_FIXED, &COLLISION_BANKS);

#[test]
fn c_header_record_is_exact_and_deterministic_when_rendered_twice() -> TestResult {
    // Given: a non-PDMA record with gaps and both byte orders.
    let exporter = RecordCHeaderExporter::new("packet.fragment.h", "Lab-9 packet");

    // When: the record is exported twice through the format-neutral interface.
    let first = render(&exporter, &RECORD)?;
    let second = render(&exporter, &RECORD)?;

    // Then: both owned UTF-8 artifacts exactly match the reviewed golden fragment.
    assert_eq!(first, second);
    assert_eq!(first.logical_name, "packet.fragment.h");
    assert_eq!(first.media_type, "text/x-c; charset=utf-8");
    assert_eq!(first.contents, include_bytes!("golden/c_header_record.h"));
    Ok(())
}

#[test]
fn c_header_register_map_is_exact_when_fixed_and_repeated_registers_are_present() -> TestResult {
    // Given: a typed non-PDMA map with fixed registers and a two-instance bank.
    let exporter = RegisterMapCHeaderExporter::new("map.fragment.h", "Lab-9 map");

    // When: the map is exported through the format-neutral interface.
    let artifact = render(&exporter, &MAP)?;

    // Then: exact metadata and expanded offsets match the reviewed golden fragment.
    assert_eq!(artifact.logical_name, "map.fragment.h");
    assert_eq!(
        artifact.contents,
        include_bytes!("golden/c_header_register_map.h")
    );
    Ok(())
}

#[test]
fn c_header_register_map_includes_independently_declared_fixed_groups() -> TestResult {
    // Given: a map whose fixed registers are supplied through a reusable declaration group.
    let exporter = RegisterMapCHeaderExporter::new("grouped.fragment.h", "grouped map");

    // When: the grouped map is exported through the generic C backend.
    let artifact = render(&exporter, &GROUPED_MAP)?;

    // Then: each grouped register contributes its offset and metadata to the fragment.
    let fragment = std::str::from_utf8(&artifact.contents)?;
    assert!(fragment.contains("C_GROUPEDQX20MAP_C_STATUS_OFFSET UINT64_C(0)"));
    assert!(fragment.contains("C_GROUPEDQX20MAP_C_COMMAND_OFFSET UINT64_C(8)"));
    Ok(())
}

#[test]
fn c_header_symbol_prefix_is_portable_when_caller_name_contains_special_characters() -> TestResult {
    // Given: a caller prefix beginning with a digit and containing repeated punctuation.
    let exporter = RecordCHeaderExporter::new("special", "9 weird--name!");

    // When: the record crosses the generic C naming boundary.
    let artifact = render(&exporter, &RECORD)?;

    // Then: the fragment uses the deterministic sanitized C identifier.
    assert!(
        artifact
            .contents
            .starts_with(b"#define C_9QX20WEIRDQX2DQX2DNAMEQX21_SIZE ")
    );
    Ok(())
}

#[test]
fn c_header_fragments_compile_and_run_when_wrapped_as_strict_c11() -> TestResult {
    // Given: both generic fragments in a caller-owned C11 translation unit.
    let record = render(
        &RecordCHeaderExporter::new("record", "Lab-9 packet"),
        &RECORD,
    )?;
    let register_map = render(&RegisterMapCHeaderExporter::new("map", "Lab-9 map"), &MAP)?;
    // When: a real C compiler builds the wrapper and the resulting binary runs.
    compile_fragments(
        "compile",
        &[&record.contents, &register_map.contents],
        HAPPY_C_MAIN,
    )?;

    // Then: compilation is warning-free and runtime byte-order/macro checks pass.
    Ok(())
}

#[test]
fn c_header_disambiguates_normalization_collisions_when_compiled_as_strict_c11() -> TestResult {
    // Given: valid record and map names that collapse at the C identifier boundary.
    let record_exporter = RecordCHeaderExporter::new("collision.record", "collision record");
    let map_exporter = RegisterMapCHeaderExporter::new("collision.map", "collision map");
    let record = render(&record_exporter, &COLLISION_RECORD)?;
    let map = render(&map_exporter, &COLLISION_MAP)?;
    assert_eq!(record, render(&record_exporter, &COLLISION_RECORD)?);
    assert_eq!(map, render(&map_exporter, &COLLISION_MAP)?);
    assert_eq!(
        record.contents,
        include_bytes!("golden/c_header_collision_record.h")
    );
    assert_eq!(
        map.contents,
        include_bytes!("golden/c_header_collision_map.h")
    );
    // When: the collision-bearing fragments are compiled and their C checks execute.
    compile_fragments(
        "collision",
        &[&record.contents, &map.contents],
        COLLISION_C_MAIN,
    )?;

    // Then: declaration-order suffixes retain distinct helpers and macro values.
    Ok(())
}

#[test]
fn c_header_keeps_independent_prefixes_collision_free_in_one_translation_unit() -> TestResult {
    // Given: separately rendered fragments whose punctuation and case used to normalize equally.
    let punctuation = RecordCHeaderExporter::new("punctuation", "a-b");
    let underscore = RecordCHeaderExporter::new("underscore", "a_b");
    let uppercase = RecordCHeaderExporter::new("uppercase", "A_B");
    let first = render(&punctuation, &RECORD)?;
    let second = render(&underscore, &RECORD)?;
    let third = render(&uppercase, &RECORD)?;
    assert_eq!(first, render(&punctuation, &RECORD)?);

    // When: all independently rendered fragments share one strict C11 translation unit.
    compile_fragments(
        "prefix-collision",
        &[&first.contents, &second.contents, &third.contents],
        PREFIX_C_MAIN,
    )?;

    // Then: each caller prefix owns a distinct deterministic C namespace.
    assert_ne!(first.contents, second.contents);
    assert_ne!(second.contents, third.contents);
    Ok(())
}
