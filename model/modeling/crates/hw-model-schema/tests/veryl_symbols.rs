//! Veryl validates symbols while streaming each package.
#![cfg(feature = "export-veryl-layout")]
/// Caller-owned output capture for exporter tests.
pub mod output_support;
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::SchemaSet,
    bits_schema,
    export::{
        ExportError,
        backends::veryl::{VerylExportError, VerylRegisterBackend},
    },
    register_schema,
};
use output_support::MemoryOutput;

bits_schema! { pub bits Flags(u32) { mode: 0..=1 => enum { variants: { Bits = 0, Active = 1 } } } }
register_schema! { pub register Status {
    width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: Flags,
} }

#[test]
fn enum_name_cannot_overwrite_generated_field_metadata() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an enum discriminant and field-width constant normalize to VALUE_MODE_BITS.
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let mut sink = MemoryOutput::new();
    // When: the package is rendered.
    let result = sink.generate(
        &AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?,
        VerylRegisterBackend::new(),
    );
    // Then: the failing package is discarded by this caller, retaining the completed dependency.
    assert!(
        matches!(result, Err(ExportError::Render(VerylExportError::Name(name))) if name == "VALUE_MODE_BITS")
    );
    let [dependency] = sink.outputs() else {
        return Err("expected one completed dependency".into());
    };
    assert_eq!(dependency.metadata.logical_name, "FlagsLayout.veryl");
    Ok(())
}

#[test]
fn package_alias_collision_prevents_all_output() -> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::ast::DefinitionId;
    // Given: two prepared definitions alias the same target package.
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let renderer = VerylRegisterBackend::new()
        .with_name(
            prepared
                .definition(DefinitionId::of::<Flags>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "Shared",
        )
        .with_name(
            prepared
                .definition(DefinitionId::of::<Status>())
                .ok_or("missing definition")?
                .id
                .clone(),
            "Shared",
        );
    let mut bytes = Vec::new();
    // When: common generation renders the complete request.
    let result = hw_model_schema::export::generate(&prepared, renderer, &mut bytes);
    // Then: metadata preflight rejects the later alias before publishing any package.
    assert!(
        matches!(result, Err(ExportError::Render(VerylExportError::Name(name))) if name == "Shared")
    );
    assert!(bytes.is_empty());
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
    let mut sink = MemoryOutput::new();
    // When: the whole-map handler emits the checked expanded registers.
    sink.generate(&prepared, VerylRegisterBackend::new())?;
    // Then: whole-node emission retains one set of metadata per placed register.
    let artifact = sink
        .outputs()
        .iter()
        .find(|artifact| artifact.metadata.logical_name == "ControlsLayout.veryl")
        .ok_or("missing map")?;
    let text = std::str::from_utf8(&artifact.bytes)?;
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
    let mut sink = MemoryOutput::new();
    // When: the independent record renderer consumes the semantic tree.
    sink.generate(&prepared, VerylRecordBackend::new())?;
    // Then: both packages retain their checked extents and bit geometry.
    let packet = sink
        .outputs()
        .iter()
        .find(|item| item.metadata.logical_name == "PacketLayout.veryl")
        .ok_or("missing record package")?;
    let packet = std::str::from_utf8(&packet.bytes)?;
    assert!(packet.contains("EXTENT: u64 = 8"));
    assert!(packet.contains("ALIGNMENT: u64 = 8"));
    let bits = sink
        .outputs()
        .iter()
        .find(|item| item.metadata.logical_name == "ModeLayout.veryl")
        .ok_or("missing bits package")?;
    let bits = std::str::from_utf8(&bits.bytes)?;
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
    let mut sink = MemoryOutput::new();
    // When: the register backend emits its geometry function.
    sink.generate(&prepared, VerylRegisterBackend::new())?;
    // Then: the completed empty extent is independent of the unused parameter.
    let [artifact] = sink.outputs() else {
        return Err("one package expected".into());
    };
    let text = std::str::from_utf8(&artifact.bytes)?;
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
    let mut sink = MemoryOutput::new();
    // When: both independent backends render through the common documentation traversal.
    sink.generate(
        &AnalyzedSchema::from_graph(&records.resolve()?, &Bindings::default())?,
        VerylRecordBackend::new(),
    )?;
    sink.generate(
        &AnalyzedSchema::from_graph(&registers.resolve()?, &Bindings::default())?,
        VerylRegisterBackend::new(),
    )?;
    // Then: every supplied line remains a line comment, including both description kinds.
    for (artifact, domain) in sink.outputs().iter().zip(["record", "register"]) {
        let text = std::str::from_utf8(&artifact.bytes)?;
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
    assert_eq!(sink.outputs().len(), 2);
    Ok(())
}

#[test]
fn writer_failure_stops_before_later_symbol_collision() -> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::export::generate;
    use std::error::Error;
    // Given: a fixed-capacity writer and a schema with a later symbol collision.
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let mut bytes = [0; 20];
    let mut output = bytes.as_mut_slice();
    // When: the opening package bytes exhaust the caller's writer.
    let result = generate(&prepared, VerylRegisterBackend::new(), &mut output);
    // Then: the write error wins over the later render error and retains its source.
    let Err(ExportError::Render(error @ VerylExportError::Io(_))) = result else {
        return Err("expected streaming write error".into());
    };
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .ok_or("missing IO source")?;
    assert_eq!(source.kind(), std::io::ErrorKind::WriteZero);
    assert_eq!(&bytes, b"// Generated schema ");
    Ok(())
}
