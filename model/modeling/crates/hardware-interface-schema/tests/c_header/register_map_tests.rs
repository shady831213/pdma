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
const COLLISION_MEMBERS_SECOND: [RegisterSpec<u8>; 2] = [
    RegisterSpec::new(5, "a-b", 0, 1),
    RegisterSpec::new(6, "a_b", 1, 1),
];
const COLLISION_BANKS: [RepeatedRegisterBank<u8>; 2] = [
    RepeatedRegisterBank::new("bank-a", 0x20, 1, 4, &COLLISION_MEMBERS),
    RepeatedRegisterBank::new("bank_a", 0x40, 1, 4, &COLLISION_MEMBERS_SECOND),
];
const COLLISION_MAP: RegisterMapSpec<u8> =
    RegisterMapSpec::new(0x80, &COLLISION_FIXED, &COLLISION_BANKS);

#[test]
fn c_header_register_map_is_exact_when_fixed_and_repeated_registers_are_present() -> TestResult {
    // Given: a typed non-PDMA map with fixed registers and a two-instance bank.
    let exporter = RegisterMapCHeaderExporter::new("map.fragment.h", "Lab-9 map");

    // When: the map is exported through the format-neutral interface.
    let artifact = render(&exporter, &MAP.resolve()?)?;

    // Then: exact metadata and expanded offsets match the reviewed golden fragment.
    assert_eq!(artifact.logical_name, "map.fragment.h");
    assert_eq!(artifact.contents, MAP_GOLDEN);
    Ok(())
}

#[test]
fn resolved_bank_retains_single_instance_topology_and_expanded_rows() -> TestResult {
    // Given: two one-instance banks whose second template member has a nonzero offset.
    let resolved = COLLISION_MAP.resolve()?;

    // When: an exporter traverses the resolved bank topology.
    let banks = resolved.banks();

    // Then: base, count, stride, templates, and expanded rows remain explicit.
    let [first, _second] = banks else {
        return Err("expected two resolved banks".into());
    };
    let [_first_template, second_template] = first.registers() else {
        return Err("expected two bank templates".into());
    };
    let [_first_expanded, second_expanded] = first.expanded_registers() else {
        return Err("expected two expanded registers".into());
    };
    assert_eq!(first.path(), "bank-a");
    assert_eq!((first.base(), first.count(), first.stride()), (32, 1, 4));
    assert_eq!(second_template.offset(), 1);
    assert_eq!(second_expanded.path(), "bank-a[0].a_b");
    assert_eq!(second_expanded.offset(), 33);
    Ok(())
}

#[test]
fn c_header_register_map_includes_independently_declared_fixed_groups() -> TestResult {
    // Given: a map whose fixed registers are supplied through a reusable declaration group.
    let exporter = RegisterMapCHeaderExporter::new("grouped.fragment.h", "grouped map");

    // When: the grouped map is exported through the generic C backend.
    let artifact = render(&exporter, &GROUPED_MAP.resolve()?)?;

    // Then: each grouped register contributes its offset and metadata to the fragment.
    let fragment = std::str::from_utf8(&artifact.contents)?;
    assert!(fragment.contains("C_GROUPEDQX20MAP_C_STATUS_OFFSET UINT64_C(0)"));
    assert!(fragment.contains("C_GROUPEDQX20MAP_C_COMMAND_OFFSET UINT64_C(8)"));
    Ok(())
}

#[test]
fn c_header_fragments_compile_and_run_when_wrapped_as_strict_c11() -> TestResult {
    // Given: both generic fragments in a caller-owned C11 translation unit.
    let record = render(
        &RecordCHeaderExporter::new("record", "Lab-9 packet"),
        &RECORD.resolve()?,
    )?;
    let register_map = render(
        &RegisterMapCHeaderExporter::new("map", "Lab-9 map"),
        &MAP.resolve()?,
    )?;
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
    let collision_record = COLLISION_RECORD.resolve()?;
    let collision_map = COLLISION_MAP.resolve()?;
    let record = render(&record_exporter, &collision_record)?;
    let map = render(&map_exporter, &collision_map)?;
    assert_eq!(record, render(&record_exporter, &collision_record)?);
    assert_eq!(map, render(&map_exporter, &collision_map)?);
    assert_eq!(record.contents, COLLISION_RECORD_GOLDEN);
    assert_eq!(map.contents, COLLISION_MAP_GOLDEN);
    // When: the collision-bearing fragments are compiled and their C checks execute.
    compile_fragments(
        "collision",
        &[&record.contents, &map.contents],
        COLLISION_C_MAIN,
    )?;

    // Then: declaration-order suffixes retain distinct helpers and macro values.
    Ok(())
}
