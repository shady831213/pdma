//! Consumers can render a graph without knowing how roots were discovered.
#![cfg(feature = "export-c-header")]
use hw_model_schema::export::backends::c_header::CHeaderExporter;
use hw_model_schema::{
    export::{Exporter, InMemoryArtifactSink},
    graph::SchemaSet,
};
use hw_model_schema::{record_schema, union_schema};

record_schema! { pub record Payload { alignment: 4, endian: Little, fields: { value: u32 } } }
union_schema! { pub union Message { cases: { Payload(Payload) } } }

mod left {
    hw_model_schema::record_schema! { pub record Entry { endian: Little, fields: { left: u32 } } }
}
mod right {
    hw_model_schema::record_schema! { pub record Entry { endian: Little, fields: { right: u32 } } }
}

#[test]
fn distinct_same_named_types_require_an_explicit_backend_name_override()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two unrelated Rust definitions share a short name.
    let mut set = SchemaSet::new();
    set.add_record::<left::Entry>();
    set.add_record::<right::Entry>();
    let graph = set.resolve()?;
    // When: the default backend would emit the same C prefix twice.
    let mut sink = InMemoryArtifactSink::new();
    let mut exporter = CHeaderExporter::new();
    let failure = exporter.export(&graph, &mut sink);
    // Then: no partial header is emitted; identity-based renaming resolves the collision.
    assert!(matches!(
        failure,
        Err(hw_model_schema::export::ExportError::Render(
            hw_model_schema::export::backends::c_header::CHeaderGraphError::Render(_)
        ))
    ));
    assert!(sink.artifacts().is_empty());
    exporter.set_name(
        hw_model_schema::graph::DefinitionId::of::<right::Entry>(),
        "right_entry",
    );
    exporter.export(&graph, &mut sink)?;
    assert_eq!(sink.artifacts().len(), 2);
    Ok(())
}

#[test]
fn c_header_exports_reachable_records_once() -> Result<(), Box<dyn std::error::Error>> {
    // Given: only the outer union is added to a manual graph.
    let mut set = SchemaSet::new();
    set.add_record::<Message>();
    let graph = set.resolve()?;
    // When: the built-in C backend renders its complete dependency set.
    let mut sink = InMemoryArtifactSink::new();
    CHeaderExporter::new().export(&graph, &mut sink)?;
    // Then: both storage geometry and the referenced record's accessors are present.
    let output = sink
        .artifacts()
        .iter()
        .map(|artifact| String::from_utf8_lossy(&artifact.contents))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(output.matches("#define C_PAYLOAD_SIZE ").count(), 1);
    assert!(output.contains("C_PAYLOAD_LOAD_C_VALUE"));
    assert!(output.contains("#define C_MESSAGE_SIZE "));
    Ok(())
}
