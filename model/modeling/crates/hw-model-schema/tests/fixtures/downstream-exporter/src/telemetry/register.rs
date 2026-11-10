use his::{
    analysis::AnalyzedSchema,
    ast::{DefinitionId, RegisterLocation},
    export::{
        register::{self, Context},
        value::{self, Emit as _},
        view,
    },
    traversal::NodeId,
};
use std::io::{self, Write};

/// Application-owned register report renderer using the prepared address projection.
#[derive(Debug)]
pub struct TelemetryRegisterRenderer {
    selected: NodeId,
    first: bool,
}
impl TelemetryRegisterRenderer {
    /// Selects the application's map without giving rendering callbacks the semantic tree.
    pub fn new(schema: &AnalyzedSchema) -> io::Result<Self> {
        Ok(Self {
            selected: schema
                .definition(DefinitionId::of::<crate::TelemetryMap>())
                .ok_or_else(|| io::Error::other("selected map"))?
                .id
                .clone(),
            first: true,
        })
    }
}
impl register::Backend for TelemetryRegisterRenderer {
    fn entry(&mut self, ctx: Context<'_>, w: &mut dyn Write) -> io::Result<()> {
        if ctx.entry_id() == &self.selected {
            self.emit(ctx.current(), w)?;
        }
        Ok(())
    }
    fn register_map<'a>(
        &mut self,
        map: view::Map<'a>,
        ctx: Context<'a>,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        write!(
            w,
            "map_size={};registers=",
            map.extent().ok_or_else(|| io::Error::other("bound map"))?
        )?;
        self.first = true;
        self.emit_all(ctx.projected_children(), w)
    }
    fn register<'a>(
        &mut self,
        _: view::Register<'a>,
        ctx: Context<'a>,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        if matches!(
            ctx.register_location(),
            RegisterLocation::Root | RegisterLocation::Template { .. }
        ) {
            return Ok(());
        }
        let site = ctx
            .register_site()
            .ok_or_else(|| io::Error::other("map register site"))?;
        let path = site
            .segments
            .iter()
            .map(|part| {
                part.index
                    .map_or_else(|| part.name.clone(), |i| format!("{}[{i}]", part.name))
            })
            .collect::<Vec<_>>()
            .join(".");
        if !self.first {
            w.write_all(b",")?;
        }
        self.first = false;
        write!(w, "{path}@{}", site.offset)
    }
}
impl value::Backend for TelemetryRegisterRenderer {
    type Domain = register::Domain;
    type Error = io::Error;
    type Context<'a> = Context<'a>;
}
