//! Cross-crate discovery plus an application-defined dependency-graph backend.
use schema::{
    ast::{
        Node, SchemaRegistry,
        analysis::{Analysis, Bindings, UnitId},
    },
    export::{Artifact, Backend, InMemoryArtifactSink, Scope, generate},
    record_schema,
};
use std::{error::Error, fmt::Write};

record_schema! {
    pub record Entry { alignment: 4, endian: Little, fields: {
        left: schema_peer::messages::Entry,
        right: schema_peer::messages::Entry,
    } }
}

/// The renderer needs only prepared references; no builtin backend is enabled.
struct DependencyGraph {
    text: String,
}
impl Backend for DependencyGraph {
    type Error = std::fmt::Error;
    fn definition(&mut self, node: Node<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        let path = match &scope.unit.id {
            UnitId::Definition(id) => id.path(),
            UnitId::View(name) => name,
        };
        writeln!(self.text, "  {path:?};")?;
        for edge in node.references() {
            writeln!(
                self.text,
                "  {path:?} -> {:?} [label={:?}];",
                edge.target().id().path(),
                edge.name()
            )?;
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Vec<Artifact>, Self::Error> {
        self.text.push_str("}\n");
        Ok(vec![Artifact {
            logical_name: "dependencies.dot".into(),
            media_type: "text/vnd.graphviz".into(),
            contents: self.text.into_bytes(),
        }])
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
    let schema = Analysis::from_graph(&graph, &Bindings::new())?;
    generate(
        &schema,
        DependencyGraph {
            text: "digraph schemas {\n".into(),
        },
        &mut sink,
    )?;
    let [artifact] = sink.artifacts() else {
        return Err("one graph artifact expected".into());
    };
    let text = std::str::from_utf8(&artifact.contents)?;
    assert_eq!(text.matches(" -> ").count(), 2);
    print!("{text}");
    Ok(())
}
