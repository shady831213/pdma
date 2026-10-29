//! The backend translates generic bank rules rather than application formulas.
#![cfg(feature = "export-veryl-layout")]
use hw_model_schema::export::{
    Exporter, InMemoryArtifactSink, backends::veryl::VerylLayoutExporter,
};
use hw_model_schema::{graph::SchemaSet, register_map, register_schema};

register_schema! { pub register Word { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: {} } }
register_map! { pub map Child { alignment: 64, placements: [pub value: Word] } }
register_map! { pub map Root(count: usize) { alignment: 64, placements: [pub children: Child[count]] } }
register_schema! { pub register BigWord { width: 2, endian: Big, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {} } }
struct LargeAddressMap;
impl hw_model_schema::graph::DescribeSchema for LargeAddressMap {
    fn schema_definition() -> hw_model_schema::graph::Definition {
        use hw_model_schema::graph::{Definition, LayoutExpr, MapDefinition, MapPlacement};
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
    let mut sink = InMemoryArtifactSink::new();
    VerylLayoutExporter::new().export(&set.resolve()?, &mut sink)?;
    // Then: the backend preserves the address; bus-width restrictions belong to the consumer.
    let artifact = sink
        .artifacts()
        .iter()
        .find(|item| item.logical_name == "LargeAddressMapLayout.veryl")
        .ok_or("missing map")?;
    assert!(std::str::from_utf8(&artifact.contents)?.contains("WORD_OFFSET: u64 = 4294967296"));
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
    let mut sink = InMemoryArtifactSink::new();
    VerylLayoutExporter::new().export(&graph, &mut sink)?;
    // Then: the target describes the original register instead of imposing PDMA restrictions.
    let [artifact] = sink.artifacts() else {
        return Err("one package expected".into());
    };
    let source = std::str::from_utf8(&artifact.contents)?;
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
    let mut sink = InMemoryArtifactSink::new();
    VerylLayoutExporter::new().export(&graph, &mut sink)?;
    // Then: output retains the parameter and references the child package geometry.
    let root = sink
        .artifacts()
        .iter()
        .find(|artifact| artifact.logical_name == "RootLayout.veryl")
        .ok_or("missing root")?;
    let source = String::from_utf8_lossy(&root.contents);
    assert!(source.contains("p_count: input u64"));
    assert!(source.contains("ChildLayout::ALIGNMENT"));
    assert!(source.contains("children_stride"));
    assert!(!source.contains("PDMA"));
    Ok(())
}
