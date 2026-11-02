//! Cross-crate discovery plus an application-defined dependency-graph backend.
use schema::{
    ast::{SchemaGraph, SchemaRegistry},
    export::Artifact,
    record_schema,
};
use std::{error::Error, fmt::Write};

record_schema! {
    pub record Entry { alignment: 4, endian: Little, fields: {
        left: schema_peer::messages::Entry,
        right: schema_peer::messages::Entry,
    } }
}

/// Dependency graph rendering consumes reusable schema identities directly.
fn dependency_graph(graph: &SchemaGraph) -> Result<Artifact, std::fmt::Error> {
    let mut text = String::from("digraph schemas {\n");
    for definition in graph.definitions() {
        let path = definition.id().path();
        writeln!(text, "  {path:?};")?;
        for edge in definition.node().references() {
            writeln!(
                text,
                "  {path:?} -> {:?} [label={:?}];",
                edge.target().id().path(),
                edge.name()
            )?;
        }
    }
    text.push_str("}\n");
    Ok(Artifact {
        logical_name: "dependencies.dot".into(),
        media_type: "text/vnd.graphviz".into(),
        contents: text.into_bytes(),
    })
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
    let artifact = dependency_graph(&graph)?;
    let text = std::str::from_utf8(&artifact.contents)?;
    assert_eq!(text.matches(" -> ").count(), 2);
    print!("{text}");
    Ok(())
}
