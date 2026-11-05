//! Cross-crate discovery plus an application-defined dependency-graph backend.
use schema::{
    ast::{SchemaGraph, SchemaRegistry},
    record_schema,
};
use std::{
    error::Error,
    io::{self, Write},
};

record_schema! {
    pub record Entry { alignment: 4, endian: Little, fields: {
        left: schema_peer::messages::Entry,
        right: schema_peer::messages::Entry,
    } }
}

/// Dependency graph rendering consumes reusable schema identities directly.
fn dependency_graph(graph: &SchemaGraph, writer: &mut dyn Write) -> io::Result<()> {
    writer.write_all(b"digraph schemas {\n")?;
    for definition in graph.definitions() {
        let path = definition.id().path();
        writeln!(writer, "  {path:?};")?;
        for edge in definition.node().references() {
            writeln!(
                writer,
                "  {path:?} -> {:?} [label={:?}];",
                edge.target().id().path(),
                edge.name()
            )?;
        }
    }
    writer.write_all(b"}\n")
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
    let mut bytes = Vec::new();
    dependency_graph(&graph, &mut bytes)?;
    let text = std::str::from_utf8(&bytes)?;
    assert_eq!(text.matches(" -> ").count(), 2);
    print!("{text}");
    Ok(())
}
