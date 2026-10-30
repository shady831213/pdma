//! Veryl rejects collisions in emitted declarations before exposing artifacts.
#![cfg(feature = "export-veryl-layout")]
use hw_model_schema::{
    bits_schema,
    export::{
        Bindings, ExportError, InMemoryArtifactSink, PreparedSchema,
        backends::veryl::{VerylExportError, VerylRenderer},
        generate,
    },
    graph::SchemaSet,
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
        &PreparedSchema::from_graph(&set.resolve()?, &Bindings::default())?,
        VerylRenderer::new(),
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
    use hw_model_schema::graph::DefinitionId;
    // Given: two prepared definitions alias the same target package.
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let prepared = PreparedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let renderer = VerylRenderer::new()
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
fn visitor_emits_fields_and_enums_once_for_each_expanded_register()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a fixed bank whose register has a bit field and enum children.
    let mut set = SchemaSet::new();
    set.add::<Controls>();
    let prepared = PreparedSchema::from_graph(&set.resolve()?, &Bindings::default())?;
    let mut sink = InMemoryArtifactSink::new();
    // When: common generation visits the definition and its expanded instances.
    generate(&prepared, VerylRenderer::new(), &mut sink)?;
    // Then: callback ordering retains one set of metadata per placed register.
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
