//! Cross-crate discovery plus an application-defined dependency-graph backend.
use schema::{
    export::{Artifact, ArtifactSink, ExportError, Exporter, InMemoryArtifactSink},
    graph::{SchemaGraph, SchemaRegistry},
    record_schema,
};
use std::{error::Error, fmt::Write};

record_schema! {
    pub record Entry { alignment: 4, endian: Little, fields: {
        left: schema_peer::messages::Entry,
        right: schema_peer::messages::Entry,
    } }
}

/// The backend needs only the public graph and artifact interfaces; no builtin backend is enabled.
struct DependencyGraph;
impl Exporter<SchemaGraph> for DependencyGraph {
    type Error = std::fmt::Error;
    fn export<Sink: ArtifactSink>(
        &self,
        graph: &SchemaGraph,
        sink: &mut Sink,
    ) -> Result<(), ExportError<Self::Error, Sink::Error>> {
        let mut text = String::from("digraph schemas {\n");
        for node in graph.definitions() {
            writeln!(text, "  {:?};", node.id().path()).map_err(ExportError::Render)?;
            for edge in node.references() {
                writeln!(
                    text,
                    "  {:?} -> {:?} [label={:?}];",
                    node.id().path(),
                    edge.target().id().path(),
                    edge.name()
                )
                .map_err(ExportError::Render)?;
            }
        }
        text.push_str("}\n");
        sink.emit(Artifact {
            logical_name: "dependencies.dot".into(),
            media_type: "text/vnd.graphviz".into(),
            contents: text.into_bytes(),
        })
        .map_err(ExportError::Sink)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Selecting this project adds a referenced peer definition, but not every peer type.
    let registry = SchemaRegistry::discover();
    let mut selected = registry.select_module("schema_discovery_fixture");
    selected.add_record::<Entry>();
    let graph = selected.resolve()?;
    assert_eq!(graph.definitions().count(), 2);
    assert_eq!(graph.roots().len(), 1);
    // The peer's unreferenced schema is nevertheless discoverable when requested.
    let peer = registry.select_module("schema_peer").resolve()?;
    assert_eq!(peer.definitions().count(), 2);
    assert!(peer.definitions().any(|node| node.name() == "Dormant"));
    let mut sink = InMemoryArtifactSink::new();
    DependencyGraph.export(&graph, &mut sink)?;
    let [artifact] = sink.artifacts() else {
        return Err("one graph artifact expected".into());
    };
    let text = std::str::from_utf8(&artifact.contents)?;
    assert_eq!(text.matches(" -> ").count(), 2);
    print!("{text}");
    Ok(())
}
