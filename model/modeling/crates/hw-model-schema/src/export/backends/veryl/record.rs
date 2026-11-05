use super::{VerylExportError, VerylRecordBackend};
use crate::traversal::record::Context;
use crate::{
    analysis::Layout,
    ast::{RecordDefinition, UnionDefinition},
    export::{record::Backend, value::Backend as _},
};
use std::io::Write;
impl Backend for VerylRecordBackend {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        self.output.begin(ctx.unit(), writer)?;
        self.emit(ctx.current(), writer)?;
        writer.write_all(b"}\n")?;
        Ok(())
    }
    fn record<'a>(
        &mut self,
        _: &'a RecordDefinition,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.extent(&ctx, writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn union<'a>(
        &mut self,
        _: &'a UnionDefinition,
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
        if ctx.field().is_none()
            && let Layout::Record(record) = &ctx.unit().body
        {
            self.output
                .package
                .extent(record.size(), record.alignment(), writer)?;
        }
        Ok(())
    }
}
