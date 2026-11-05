//! A custom register backend using no builtin backend feature.
use hw_model_schema::analysis::{AnalyzedSchema, Bindings, RegisterMapView};
use hw_model_schema::export::generate;
use hw_model_schema::export::value::Backend as _;
use hw_model_schema::traversal::register::Context as RegisterContext;
use hw_model_schema::{
    ast::{DefinitionId, RegisterInfo, RegisterLocation, SchemaSet},
    export::{Artifact, InMemoryArtifactSink, register::Backend, value},
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
    selected: hw_model_schema::traversal::NodeId,
    text: String,
}
impl Backend for RegisterListing {
    fn root(&mut self, ctx: RegisterContext<'_>) -> Result<(), Self::Error> {
        if ctx.unit().id == self.selected {
            self.emit(ctx.current())?;
        }
        Ok(())
    }
    fn register_map<'a>(
        &mut self,
        _: &'a RegisterMapView,
        ctx: RegisterContext<'a>,
    ) -> Result<(), Self::Error> {
        self.text.push_str("path\toffset\tbytes\n");
        self.emit_all(ctx.children())
    }
    fn register(
        &mut self,
        register: &RegisterInfo,
        ctx: RegisterContext<'_>,
    ) -> Result<(), Self::Error> {
        match ctx.register_location() {
            RegisterLocation::Root | RegisterLocation::Template { .. } => return Ok(()),
            RegisterLocation::Standalone(_) | RegisterLocation::Instance { .. } => {}
        }
        let site = ctx.register_site().ok_or("map register site expected")?;
        let path = site
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
        writeln!(self.text, "{path}\t{}\t{}", site.offset, register.width())?;
        Ok(())
    }
}
impl value::Backend for RegisterListing {
    type Domain = hw_model_schema::export::register::Domain;
    type Error = Box<dyn Error>;
    type Context<'a> = RegisterContext<'a>;
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
    let schema = AnalyzedSchema::from_graph(&selected.resolve()?, &bindings)?;
    let backend = RegisterListing {
        selected: schema
            .definition(DefinitionId::of::<Device>())
            .ok_or("missing device")?
            .id
            .clone(),
        text: String::new(),
    };
    let mut sink = InMemoryArtifactSink::new();
    generate(&schema, backend, &mut sink).map_err(|error| format!("{error}"))?;
    for artifact in sink.into_artifacts() {
        print!("{}", String::from_utf8(artifact.contents)?);
    }
    Ok(())
}
