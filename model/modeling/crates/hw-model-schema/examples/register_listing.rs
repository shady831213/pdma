//! A custom register backend using no builtin backend feature.
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::export::generate;
use hw_model_schema::export::register::Context as RegisterContext;
use hw_model_schema::export::value::Emit as _;
use hw_model_schema::{
    ast::{DefinitionId, RegisterLocation, SchemaSet},
    export::{
        OutputMetadata,
        register::Backend,
        value,
        view::{Map, Register},
    },
    register_map, register_schema,
};
use std::{
    error::Error,
    io::{self, Write},
};

register_schema! {
    pub register Status { width: 4, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0, fields: {} }
}
register_map! { pub map Channel { alignment: 16, placements: [pub status: Status] } }
register_map! { pub map Device(n: usize) { alignment: 16, placements: [pub channels: Channel[n]] } }

struct RegisterListing {
    selected: hw_model_schema::traversal::NodeId,
}
impl Backend for RegisterListing {
    fn root(
        &mut self,
        ctx: RegisterContext<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.root_id() == &self.selected {
            self.emit(ctx.current(), writer)?;
        }
        Ok(())
    }
    fn register_map<'a>(
        &mut self,
        _: Map<'a>,
        ctx: RegisterContext<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        writer.write_all(b"path\toffset\tbytes\n")?;
        self.emit_all(ctx.children(), writer)
    }
    fn register(
        &mut self,
        register: Register<'_>,
        ctx: RegisterContext<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        match ctx.register_location() {
            RegisterLocation::Root | RegisterLocation::Template { .. } => return Ok(()),
            RegisterLocation::Standalone(_) | RegisterLocation::Instance { .. } => {}
        }
        let site = ctx
            .register_site()
            .ok_or_else(|| io::Error::other("map register site expected"))?;
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
        writeln!(writer, "{path}\t{}\t{}", site.offset, register.width())?;
        Ok(())
    }
}
impl value::Backend for RegisterListing {
    type Domain = hw_model_schema::export::register::Domain;
    type Error = io::Error;
    type Context<'a> = RegisterContext<'a>;
}
impl hw_model_schema::export::OutputBackend for RegisterListing {
    fn metadata(&mut self, _: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: "registers.tsv".into(),
            media_type: "text/tab-separated-values".into(),
        })
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
    };
    let mut output = io::stdout().lock();
    generate(&schema, backend, &mut output)?;
    output.flush()?;
    Ok(())
}
