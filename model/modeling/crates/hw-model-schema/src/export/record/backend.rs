use crate::ast::{Case, Field, RecordDefinition, UnionDefinition, record::Constant};
use crate::traversal::record::Context;
/// Target syntax restricted to record source constructs.
pub trait Backend: for<'a> super::super::value::Backend<Context<'a> = Context<'a>> {
    /// Handles one selected record/value root.
    fn root(&mut self, ctx: Context<'_>) -> Result<(), Self::Error> {
        self.emit(ctx.current())
    }
    /// Handles a source record.
    fn record<'a>(
        &mut self,
        _value: &'a RecordDefinition,
        ctx: Context<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a tagged or untagged union.
    fn union<'a>(
        &mut self,
        _value: &'a UnionDefinition,
        ctx: Context<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a logical field.
    fn field<'a>(&mut self, _value: &'a Field, ctx: Context<'a>) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a union alternative.
    fn case<'a>(&mut self, _value: &'a Case, ctx: Context<'a>) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles the record size source attribute.
    fn record_size(&mut self, _: usize, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the record alignment source attribute.
    fn record_alignment(&mut self, _: usize, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the record endian source attribute.
    fn record_endian(&mut self, _: crate::Endian, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the field size source attribute.
    fn field_size(&mut self, _: usize, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the field offset source attribute.
    fn field_offset(&mut self, _: usize, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the field endian source attribute.
    fn field_endian(&mut self, _: crate::Endian, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a scalar constant constraint with its original source symbol.
    fn constant(&mut self, _: &Constant, _: Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}
