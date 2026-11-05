//! The backend translates generic bank rules rather than application formulas.
#![cfg(feature = "export-veryl-layout")]
/// Caller-owned output capture for exporter tests.
pub mod output_support;
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::export::backends::veryl::VerylRegisterBackend;
use hw_model_schema::{ast::SchemaSet, register_map, register_schema};
use output_support::MemoryOutput;

register_schema! { pub register Word { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: {} } }
register_map! { pub map Child { alignment: 64, placements: [pub value: Word] } }
register_map! { pub map Root(count: usize) { alignment: 64, placements: [pub children: Child[count]] } }
register_schema! { pub register BigWord { width: 2, endian: Big, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {} } }
struct LargeAddressMap;
impl hw_model_schema::ast::DescribeSchema for LargeAddressMap {
    fn schema_definition() -> hw_model_schema::ast::Definition {
        use hw_model_schema::ast::{Definition, LayoutExpr, MapDefinition, MapPlacement};
        // This test needs layout metadata only, not a multi-gigabyte owned value type.
        Definition::register_map::<Self>(
            "LargeAddressMap",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(
                &[],
                4,
                None,
                vec![MapPlacement::new(
                    "word",
                    <Word as hw_model_schema::RegisterLayout>::schema_definition,
                    vec![],
                    LayoutExpr::Constant(1),
                    Some(0x1_0000_0000),
                    false,
                )],
            ),
        )
    }
}

#[test]
fn generic_metadata_does_not_truncate_large_register_offsets()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a layout outside the 32-bit address profile of the PDMA instance.
    let mut set = SchemaSet::new();
    set.add::<LargeAddressMap>();
    // When: layout metadata is rendered.
    let mut sink = MemoryOutput::new();
    sink.generate(
        &AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?,
        VerylRegisterBackend::new(),
    )?;
    // Then: the backend preserves the address; bus-width restrictions belong to the consumer.
    let artifact = sink
        .outputs()
        .iter()
        .find(|item| item.metadata.logical_name == "LargeAddressMapLayout.veryl")
        .ok_or("missing map")?;
    assert!(std::str::from_utf8(&artifact.bytes)?.contains("WORD_OFFSET: u64 = 4294967296"));
    Ok(())
}

#[test]
fn layout_backend_preserves_non_pdma_register_width_and_endian()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a register whose byte order and width differ from PDMA's CSR profile.
    let mut set = SchemaSet::new();
    set.add::<BigWord>();
    let graph = set.resolve()?;
    // When: generic metadata is exported without a bus implementation.
    let mut sink = MemoryOutput::new();
    sink.generate(
        &AnalyzedSchema::from_graph(&graph, &Bindings::default())?,
        VerylRegisterBackend::new(),
    )?;
    // Then: the target describes the original register instead of imposing PDMA restrictions.
    let [artifact] = sink.outputs() else {
        return Err("one package expected".into());
    };
    let source = std::str::from_utf8(&artifact.bytes)?;
    assert!(source.contains("VALUE_BYTES: u32 = 2"));
    assert!(source.contains("VALUE_ENDIAN_LITTLE: bbool = false"));
    Ok(())
}

#[test]
fn veryl_keeps_the_count_parameter_and_child_alignment_rule()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a parameterized bank, without any application-specific layout code.
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let graph = set.resolve()?;
    // When: the generic Veryl backend emits the definition graph.
    let mut sink = MemoryOutput::new();
    sink.generate(
        &AnalyzedSchema::from_graph(&graph, &Bindings::default())?,
        VerylRegisterBackend::new(),
    )?;
    // Then: output retains the parameter and completed child geometry.
    let root = sink
        .outputs()
        .iter()
        .find(|artifact| artifact.metadata.logical_name == "RootLayout.veryl")
        .ok_or("missing root")?;
    let source = String::from_utf8_lossy(&root.bytes);
    assert!(source.contains("p_count: input u64"));
    assert!(
        source.contains("function children_offset(p_count: input u64) -> u64 {\n        return 0;")
    );
    assert!(
        source
            .contains("function children_stride(p_count: input u64) -> u64 {\n        return 64;")
    );
    assert!(source.contains("children_stride"));
    assert!(!source.contains("PDMA"));
    Ok(())
}

#[test]
fn veryl_projection_keeps_bank_offsets_and_effective_permissions()
-> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::{Access, RegisterMapSpec, RegisterSpec, RepeatedRegisterBank};
    // Given: a projected bank hides one member and removes writes from the other.
    const MEMBERS: [RegisterSpec<u8>; 2] = [
        RegisterSpec::new(1, "private", 0, 4),
        RegisterSpec::new(2, "public", 8, 4)
            .with_access(Access::ReadWrite)
            .with_writable_mask(7),
    ];
    const BANKS: [RepeatedRegisterBank<u8>; 1] =
        [RepeatedRegisterBank::new("ports", 32, 3, 16, &MEMBERS)];
    let map = RegisterMapSpec::named("sensor", 128, &[], &BANKS).resolve()?;
    let view = map.project_access(|register| (register.id() == 2).then_some(Access::ReadOnly))?;
    let prepared = AnalyzedSchema::register_map("Observer", &view)?;
    let mut sink = MemoryOutput::new();
    // When: the same renderer consumes the explicit permission view.
    sink.generate(&prepared, VerylRegisterBackend::new())?;
    // Then: concrete addresses retain bank holes and no permission is restored.
    let [artifact] = sink.outputs() else {
        return Err("one package expected".into());
    };
    let text = std::str::from_utf8(&artifact.bytes)?;
    assert!(text.contains("EXTENT: u64 = 128"));
    for (index, offset) in [(0, 40), (1, 56), (2, 72)] {
        assert!(text.contains(&format!("PORTS_{index}_PUBLIC_OFFSET: u64 = {offset}")));
        assert!(text.contains(&format!("PORTS_{index}_PUBLIC_WRITE: bbool = false")));
        assert!(text.contains(&format!(
            "PORTS_{index}_PUBLIC_WRITABLE: u64 = 64'h0000000000000000"
        )));
    }
    assert!(!text.contains("PRIVATE"));
    assert!(!text.contains("ALIGNMENT"));
    Ok(())
}

#[test]
fn veryl_bound_map_uses_prepared_geometry_and_expanded_members()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the common preparation binds an instance of the symbolic root map.
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let mut bindings = Bindings::new();
    bindings.bind::<Root>(&[2]);
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &bindings)?;
    let mut sink = MemoryOutput::new();
    // When: the same renderer receives the bound prepared schema.
    sink.generate(&prepared, VerylRegisterBackend::new())?;
    // Then: the root has no parameter and concrete register offsets retain child alignment.
    let artifact = sink
        .outputs()
        .iter()
        .find(|artifact| artifact.metadata.logical_name == "RootLayout.veryl")
        .ok_or("missing root")?;
    let text = std::str::from_utf8(&artifact.bytes)?;
    assert!(text.contains("function extent() -> u64"));
    assert!(text.contains("EXTENT: u64 = 68"));
    assert!(text.contains("CHILDREN_0_VALUE_OFFSET: u64 = 0"));
    assert!(text.contains("CHILDREN_1_VALUE_OFFSET: u64 = 64"));
    assert!(!text.contains("p_count"));
    Ok(())
}

struct NestedMaximum;
impl hw_model_schema::ast::DescribeSchema for NestedMaximum {
    fn schema_definition() -> hw_model_schema::ast::Definition {
        use hw_model_schema::ast::{Definition, LayoutExpr, MapDefinition, MapPlacement};
        Definition::register_map::<Self>(
            "NestedMaximum",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(
                &["count"],
                4,
                None,
                vec![MapPlacement::new(
                    "words",
                    <Word as hw_model_schema::RegisterLayout>::schema_definition,
                    vec![],
                    LayoutExpr::Add(
                        Box::new(LayoutExpr::Maximum(
                            Box::new(LayoutExpr::Parameter("count")),
                            Box::new(LayoutExpr::Constant(2)),
                        )),
                        Box::new(LayoutExpr::Constant(1)),
                    ),
                    None,
                    true,
                )],
            ),
        )
    }
}

#[test]
fn nested_maximum_remains_one_operand_of_addition() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the bank count is max(count, 2) + 1, with addition outside the maximum.
    let mut set = SchemaSet::new();
    set.add::<NestedMaximum>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let mut sink = MemoryOutput::new();
    // When: the real renderer formats the nested expression through common generation.
    sink.generate(&prepared, VerylRegisterBackend::new())?;
    // Then: parentheses keep addition outside both branches of the Veryl conditional.
    let artifact = sink
        .outputs()
        .iter()
        .find(|artifact| artifact.metadata.logical_name == "NestedMaximumLayout.veryl")
        .ok_or("missing nested maximum map")?;
    let text = std::str::from_utf8(&artifact.bytes)?;
    assert!(text.contains("return ((if p_count >: 2 ? p_count : 2) + 1);"));
    Ok(())
}

#[test]
fn prepared_metadata_does_not_change_root_package() -> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::export::{backends::veryl::VerylRegisterBackend, prepare};
    use hw_model_schema::{RecordSpec, ast::Definition};
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<u8>(RecordSpec::named(
        "First",
        1,
        1,
        &[],
        &[],
    )));
    set.add_definition(Definition::opaque::<u16>(RecordSpec::named(
        "Second",
        2,
        2,
        &[],
        &[],
    )));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut backend = VerylRegisterBackend::new();
    let roots = prepare(&schema, &mut backend)?;
    for root in roots {
        let expected = format!(
            "pub package {} {{",
            root.metadata().logical_name.trim_end_matches(".veryl")
        );
        let mut bytes = Vec::new();
        root.emit(&mut backend, &mut bytes)?;
        assert!(
            std::str::from_utf8(&bytes)?.contains(&expected),
            "expected {expected}"
        );
    }
    Ok(())
}
