use hw_model_schema::{
    analysis::CompleteMapLayout,
    ast::RegisterInfo,
    export::{OutputMetadata, SymbolScope, register, value},
    traversal::{NodeId, register::Context},
};
use std::io::{Error, Write};

#[derive(Default)]
pub(super) struct Uvm {
    names: SymbolScope<NodeId>,
}

impl value::Backend for Uvm {
    type Error = Error;
    type Domain = register::Domain;
    type Context<'a> = Context<'a>;

    fn metadata(&mut self, ctx: &Context<'_>) -> Result<OutputMetadata, Error> {
        self.names
            .insert(&ctx.unit().name, ctx.unit().id.clone())
            .map_err(Error::other)?;
        Ok(OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/x-systemverilog".into(),
        })
    }
}

impl register::Backend for Uvm {
    fn register<'a>(
        &mut self,
        value: &'a RegisterInfo,
        _: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        super::fields::emit(value, writer)
    }

    fn map_layout<'a>(
        &mut self,
        value: &'a CompleteMapLayout,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        super::maps::emit(value, &ctx.unit().name, writer)
    }

    fn register_map<'a>(
        &mut self,
        _: &'a hw_model_schema::analysis::RegisterMapView,
        ctx: Context<'a>,
        _: &mut dyn Write,
    ) -> Result<(), Error> {
        // Completed plans own hierarchy. Permission-only views have no reusable
        // hierarchy and must not silently emit their unprojected declarations.
        match &ctx.unit().body {
            hw_model_schema::analysis::Layout::Map(map) if map.plan.is_some() => Ok(()),
            _ => Err(Error::other(
                "permission-projected register maps are unsupported",
            )),
        }
    }
}
