use super::{VerylExportError, VerylRecordBackend};
use crate::export::record::Context;
use crate::{
    export::view::{Record, Union},
    export::{record::Backend, value::Emit as _},
};
use std::io::Write;
impl Backend for VerylRecordBackend {
    fn entry(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        self.output
            .begin(ctx.entry_id(), ctx.entry_name(), writer)?;
        self.emit(ctx.current(), writer)?;
        writer.write_all(b"}\n")?;
        Ok(())
    }
    fn reference<'a>(
        &mut self,
        _: crate::export::view::Reference<'a>,
        target: crate::export::Visit<Context<'a>>,
        _: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if target.definition_origin() == crate::ast::Origin::Inline {
            self.emit(target, writer)?;
        }
        Ok(())
    }
    fn record<'a>(
        &mut self,
        _: Record<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.extent(&ctx, writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn union<'a>(
        &mut self,
        _: Union<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.extent(&ctx, writer)?;
        self.emit_all(ctx.children(), writer)
    }
}
impl VerylRecordBackend {
    pub(super) fn extent(
        &mut self,
        ctx: &Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), VerylExportError> {
        if ctx.is_entry()
            && let Some((size, alignment)) = ctx.entry_extent()
        {
            self.output.package.extent(size, alignment, writer)?;
        }
        Ok(())
    }
}
