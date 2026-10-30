//! A custom register renderer using no builtin backend feature.
use hw_model_schema::{
    export::{
        Artifact, Bindings, DefinitionHeader, ExportId, InMemoryArtifactSink, PreparedMap,
        PreparedSchema, RegisterLocation, RegisterMapView, RegisterNode, Renderer, generate,
    },
    graph::{DefinitionId, SchemaSet},
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
    selected: ExportId,
    text: String,
    active: bool,
}
impl Renderer for RegisterListing {
    type Error = Box<dyn Error>;
    fn begin_definition(&mut self, definition: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        self.active = *definition.id == self.selected;
        Ok(())
    }
    fn begin_map(&mut self, map: &PreparedMap) -> Result<(), Self::Error> {
        if self.active && map.instance.is_none() {
            return Err("listing requires a bound map".into());
        }
        Ok(())
    }
    fn begin_map_instance(&mut self, _: &RegisterMapView) -> Result<(), Self::Error> {
        if self.active {
            self.text.push_str("path\toffset\tbytes\n");
        }
        Ok(())
    }
    fn begin_register(&mut self, node: RegisterNode<'_>) -> Result<(), Self::Error> {
        if !self.active {
            return Ok(());
        }
        match node.location {
            RegisterLocation::Root | RegisterLocation::Template { .. } => return Ok(()),
            RegisterLocation::Standalone(_) | RegisterLocation::Instance { .. } => {}
        }
        let leaf = node.view.ok_or("map register placement expected")?;
        let path = leaf
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
        writeln!(
            self.text,
            "{path}\t{}\t{}",
            leaf.offset,
            node.register.width()
        )?;
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
    let schema = PreparedSchema::from_graph(&selected.resolve()?, &bindings)?;
    let renderer = RegisterListing {
        selected: DefinitionId::of::<Device>().into(),
        text: String::new(),
        active: false,
    };
    let mut sink = InMemoryArtifactSink::new();
    generate(&schema, renderer, &mut sink).map_err(|error| format!("{error}"))?;
    for artifact in sink.into_artifacts() {
        print!("{}", String::from_utf8(artifact.contents)?);
    }
    Ok(())
}
