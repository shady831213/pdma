use super::{VerylExportError, VerylRecordBackend};
use crate::traversal::record::Context;
use crate::{
    analysis::Layout,
    ast::{RecordDefinition, UnionDefinition},
    export::{record::Backend, value::Backend as _},
};
impl Backend for VerylRecordBackend {
    fn root(&mut self, ctx: Context<'_>) -> Result<(), Self::Error> {
        let name = self.output.begin(ctx.unit())?;
        self.emit(ctx.current())?;
        self.output.end(&name);
        Ok(())
    }
    fn record<'a>(&mut self, _: &'a RecordDefinition, ctx: Context<'a>) -> Result<(), Self::Error> {
        self.extent(&ctx)?;
        self.emit_all(ctx.children())
    }
    fn union<'a>(&mut self, _: &'a UnionDefinition, ctx: Context<'a>) -> Result<(), Self::Error> {
        self.extent(&ctx)?;
        self.emit_all(ctx.children())
    }
}
impl VerylRecordBackend {
    pub(super) fn extent(&mut self, ctx: &Context<'_>) -> Result<(), VerylExportError> {
        if ctx.field().is_none()
            && let Layout::Record(record) = &ctx.unit().body
        {
            self.output
                .package
                .extent(record.size(), record.alignment())?;
        }
        Ok(())
    }
}
