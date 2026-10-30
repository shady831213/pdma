//! A custom register backend using no builtin backend feature.
use hw_model_schema::{
    ast::{
        DefinitionId, MapView, Node, PlacementView, RegisterInfo, RegisterLocation, SchemaSet,
        analysis::{Analysis, Bindings, UnitId},
    },
    export::{Artifact, Backend, InMemoryArtifactSink, Scope, emit, emit_all, generate},
    register_map, register_schema,
};
use std::{error::Error, fmt::Write};

register_schema! {
    pub register Status { width: 4, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0, fields: {} }
}
register_map! { pub map Channel { alignment: 16, placements: [pub status: Status] } }
register_map! { pub map Device(n: usize) { alignment: 16, placements: [pub channels: Channel[n]] } }

struct RegisterListing {
    selected: UnitId,
    text: String,
}
impl Backend for RegisterListing {
    type Error = Box<dyn Error>;
    fn definition(&mut self, node: Node<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        if scope.unit.id == self.selected {
            emit(node, scope, self)?;
        }
        Ok(())
    }
    fn register_map(&mut self, _: MapView<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        let map = scope
            .map_layout()
            .and_then(|layout| layout.instance.as_ref())
            .ok_or("listing requires a bound map")?;
        self.text.push_str("path\toffset\tbytes\n");
        emit_all(MapView::Resolved(map).children(), scope, self)
    }
    fn register(&mut self, register: &RegisterInfo, scope: Scope<'_>) -> Result<(), Self::Error> {
        match scope.register_location() {
            RegisterLocation::Root | RegisterLocation::Template { .. } => return Ok(()),
            RegisterLocation::Standalone(_) | RegisterLocation::Instance { .. } => {}
        }
        let Some(PlacementView::Register { view, .. }) = scope.placement else {
            return Err("map register placement expected".into());
        };
        let path = view
            .segments
            .iter()
            .map(|part| {
                part.index.map_or_else(
                    || part.name.clone(),
                    |index| format!("{}[{index}]", part.name),
                )
            })
            .collect::<Vec<_>>()
            .join(".");
        writeln!(self.text, "{path}\t{}\t{}", view.offset, register.width())?;
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "registers.tsv".into(),
            media_type: "text/tab-separated-values".into(),
            contents: self.text.into_bytes(),
        }])
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut selected = SchemaSet::new();
    selected.add::<Device>();
    let mut bindings = Bindings::new();
    bindings.bind::<Device>(&[2]);
    let schema = Analysis::from_graph(&selected.resolve()?, &bindings)?;
    let backend = RegisterListing {
        selected: DefinitionId::of::<Device>().into(),
        text: String::new(),
    };
    let mut sink = InMemoryArtifactSink::new();
    generate(&schema, backend, &mut sink).map_err(|error| format!("{error}"))?;
    for artifact in sink.into_artifacts() {
        print!("{}", String::from_utf8(artifact.contents)?);
    }
    Ok(())
}
