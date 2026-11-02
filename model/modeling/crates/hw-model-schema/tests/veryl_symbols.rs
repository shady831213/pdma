//! Veryl rejects collisions in emitted declarations before exposing artifacts.
#![cfg(feature = "export-veryl-layout")]
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::SchemaSet,
    bits_schema,
    export::{
        ExportError, InMemoryArtifactSink,
        backends::veryl::{VerylExportError, VerylRegisterBackend},
        generate,
    },
    register_schema,
};

bits_schema! { pub bits Flags(u32) { mode: 0..=1 => enum { variants: { Bits = 0, Active = 1 } } } }
register_schema! { pub register Status {
    width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: Flags,
} }

#[test]
fn enum_name_cannot_overwrite_generated_field_metadata() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an enum discriminant and field-width constant normalize to VALUE_MODE_BITS.
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let mut sink = InMemoryArtifactSink::new();
    // When: the package is rendered.
    let result = generate(
        &AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?,
        VerylRegisterBackend::new(),
        &mut sink,
    );
    // Then: the conflicting declaration is rejected without emitting a partial package.
    assert!(
        matches!(result, Err(ExportError::Render(VerylExportError::Name(name))) if name == "VALUE_MODE_BITS")
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}

#[test]
fn package_alias_collision_rejects_all_artifacts() -> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::ast::DefinitionId;
    // Given: two prepared definitions alias the same target package.
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let renderer = VerylRegisterBackend::new()
        .with_name(DefinitionId::of::<Flags>(), "Shared")
        .with_name(DefinitionId::of::<Status>(), "Shared");
    let mut sink = InMemoryArtifactSink::new();
    // When: common generation renders the complete request.
    let result = generate(&prepared, renderer, &mut sink);
    // Then: package alias collision is reported before the sink receives any artifact.
    assert!(
        matches!(result, Err(ExportError::Render(VerylExportError::Name(name))) if name == "Shared")
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}

bits_schema! { pub bits Mode(u32) { state: 4..=5 => enum { variants: { Idle = 0, Active = 1 } } } }
register_schema! { pub register Control {
    width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0x30, fields: Mode,
} }
hw_model_schema::register_map! { pub map Controls { alignment: 4, placements: [pub controls: Control[2]] } }

#[test]
fn backend_emits_fields_and_enums_once_for_each_expanded_register()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a fixed bank whose register has a bit field and enum children.
    let mut set = SchemaSet::new();
    set.add::<Controls>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let mut sink = InMemoryArtifactSink::new();
    // When: the whole-map handler emits the checked expanded registers.
    generate(&prepared, VerylRegisterBackend::new(), &mut sink)?;
    // Then: whole-node emission retains one set of metadata per placed register.
    let artifact = sink
        .artifacts()
        .iter()
        .find(|artifact| artifact.logical_name == "ControlsLayout.veryl")
        .ok_or("missing map")?;
    let text = std::str::from_utf8(&artifact.contents)?;
    for index in 0..2 {
        for suffix in [
            "LSB: u32 = 4",
            "BITS: u32 = 2",
            "IDLE: u64 = 0",
            "ACTIVE: u64 = 1",
        ] {
            assert_eq!(
                text.matches(&format!("CONTROLS_{index}_STATE_{suffix}"))
                    .count(),
                1
            );
        }
    }
    Ok(())
}

hw_model_schema::record_schema! { pub record Packet { alignment: 8, endian: Little, fields: { value: u32 } } }

#[test]
fn independent_record_backend_preserves_record_and_bits_packages()
-> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::export::backends::veryl::VerylRecordBackend;
    // Given: record and reusable bit schemas selected without registers.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    set.add::<Mode>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let mut sink = InMemoryArtifactSink::new();
    // When: the independent record renderer consumes the semantic tree.
    generate(&prepared, VerylRecordBackend::new(), &mut sink)?;
    // Then: both packages retain their checked extents and bit geometry.
    let packet = sink
        .artifacts()
        .iter()
        .find(|item| item.logical_name == "PacketLayout.veryl")
        .ok_or("missing record package")?;
    let packet = std::str::from_utf8(&packet.contents)?;
    assert!(packet.contains("EXTENT: u64 = 8"));
    assert!(packet.contains("ALIGNMENT: u64 = 8"));
    let bits = sink
        .artifacts()
        .iter()
        .find(|item| item.logical_name == "ModeLayout.veryl")
        .ok_or("missing bits package")?;
    let bits = std::str::from_utf8(&bits.contents)?;
    assert!(bits.contains("BIT_WIDTH: u32 = 6"));
    assert!(bits.contains("DECLARED_MASK: u64 = 64'h0000000000000030"));
    Ok(())
}

struct EmptyMap;
impl hw_model_schema::ast::DescribeSchema for EmptyMap {
    fn schema_definition() -> hw_model_schema::ast::Definition {
        use hw_model_schema::ast::{Definition, MapDefinition};
        Definition::register_map::<Self>(
            "EmptyMap",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(&["unused"], 4, None, vec![]),
        )
    }
}

#[test]
fn empty_parameterized_map_emits_zero_extent() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an unbound map with no placements and an implicit zero extent.
    let mut set = SchemaSet::new();
    set.add::<EmptyMap>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let mut sink = InMemoryArtifactSink::new();
    // When: the register backend emits its geometry function.
    generate(&prepared, VerylRegisterBackend::new(), &mut sink)?;
    // Then: the completed empty extent is independent of the unused parameter.
    let [artifact] = sink.artifacts() else {
        return Err("one package expected".into());
    };
    let text = std::str::from_utf8(&artifact.contents)?;
    assert!(text.contains("function extent(p_unused: input u64) -> u64"));
    assert!(text.contains("return 0;"));
    Ok(())
}

hw_model_schema::record_schema! {
    #[short_description = "record summary\nsecond */"]
    #[description = "record detail\nthird /*"]
    pub record DocumentedPacket { endian: Little, fields: { value: u32 } }
}
register_schema! {
    #[short_description = "register summary\nsecond */"]
    #[description = "register detail\nthird /*"]
    pub register DocumentedControl {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: {},
    }
}

#[test]
fn multiline_documentation_stays_inside_comments_in_both_domains()
-> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::export::backends::veryl::VerylRecordBackend;
    // Given: short and full documentation contain newlines and block-comment delimiters.
    let mut records = SchemaSet::new();
    records.add::<DocumentedPacket>();
    let mut registers = SchemaSet::new();
    registers.add::<DocumentedControl>();
    let mut sink = InMemoryArtifactSink::new();
    // When: both independent backends render through the common documentation traversal.
    generate(
        &AnalyzedSchema::from_graph(&records.resolve()?, &Bindings::default())?,
        VerylRecordBackend::new(),
        &mut sink,
    )?;
    generate(
        &AnalyzedSchema::from_graph(&registers.resolve()?, &Bindings::default())?,
        VerylRegisterBackend::new(),
        &mut sink,
    )?;
    // Then: every supplied line remains a line comment, including both description kinds.
    for (artifact, domain) in sink.artifacts().iter().zip(["record", "register"]) {
        let text = std::str::from_utf8(&artifact.contents)?;
        for line in [
            format!("{domain} summary"),
            "second */".into(),
            format!("{domain} detail"),
            "third /*".into(),
        ] {
            assert!(
                text.lines()
                    .any(|emitted| emitted.trim() == format!("// {line}"))
            );
        }
    }
    assert_eq!(sink.artifacts().len(), 2);
    Ok(())
}
