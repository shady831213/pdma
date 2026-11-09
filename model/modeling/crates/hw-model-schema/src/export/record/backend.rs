use super::Context;
use crate::ast::record::Constant;
use crate::export::{value::Emit as _, view};
/// Target syntax restricted to record source constructs.
pub trait Backend: for<'a> super::super::value::Backend<Context<'a> = Context<'a>> {
    /// Handles a reference; only this callback receives its bound target visit.
    /// Defaults to no output; explicitly emit the target to inline its definition.
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        _: crate::export::Visit<Context<'a>>,
        _: Context<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles one selected record/value root.
    fn root(
        &mut self,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit(ctx.current(), writer)
    }
    /// Handles a source record.
    fn record<'a>(
        &mut self,
        _value: view::Record<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a tagged or untagged union.
    fn union<'a>(
        &mut self,
        _value: view::Union<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a logical field.
    fn field<'a>(
        &mut self,
        _value: view::Field<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a union alternative.
    fn case<'a>(
        &mut self,
        _value: view::Case<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a scalar constant constraint with its original source symbol.
    fn constant(
        &mut self,
        _: &Constant,
        _: Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
