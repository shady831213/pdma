use super::{VerylExportError, VerylRecordBackend};
use crate::export::record::Context;
use crate::{
    export::view::{Record, Union},
    export::{record::Backend, value::Emit as _},
};
use std::io::Write;
impl Backend for VerylRecordBackend {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        self.output.begin(ctx.root_id(), ctx.root_name(), writer)?;
        self.emit(ctx.current(), writer)?;
        writer.write_all(b"}\n")?;
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
        if ctx.is_root()
            && let Some((size, alignment)) = ctx.root_extent()
        {
            self.output.package.extent(size, alignment, writer)?;
        }
        Ok(())
    }
}
