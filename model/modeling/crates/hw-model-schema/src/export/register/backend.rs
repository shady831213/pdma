use super::Context;
use crate::ast::register::Parameter;
use crate::export::{value::Emit as _, view};
/// Target syntax restricted to register source constructs.
pub trait Backend: for<'a> super::super::value::Backend<Context<'a> = Context<'a>> {
    /// Handles a type reference and its use-bound expansion visit.
    /// Defaults to expanding anonymous value types; named types remain references.
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        _: view::Target<'a>,
        target: crate::export::Visit<Context<'a>>,
        _: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if target.definition_origin() == crate::ast::Origin::Inline {
            self.emit(target, writer)?;
        }
        Ok(())
    }
    /// Handles a selected register/value entry.
    fn entry(
        &mut self,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit(ctx.current(), writer)
    }
    /// Handles a register declaration.
    fn register<'a>(
        &mut self,
        _value: view::Register<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a source or projected map.
    fn register_map<'a>(
        &mut self,
        _value: view::Map<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a completed bank with framework-selected property children.
    fn bank<'a>(
        &mut self,
        _: super::Bank<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles one final bank property.
    fn bank_property(
        &mut self,
        _: super::BankProperty,
        _: Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a permission-projected register placement.
    fn register_placement<'a>(
        &mut self,
        _: view::RegisterPlacement<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a register bit field; defaults to the shared value callback.
    /// The original occurrence, register ownership and children remain bound in ctx.
    fn register_field(
        &mut self,
        field: view::BitField,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.bit_field(field, ctx, writer)
    }
    /// Handles a source or concrete placement.
    fn placement<'a>(
        &mut self,
        _value: view::Placement<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a declared constructor parameter and optional entry binding.
    fn parameter(
        &mut self,
        _: Parameter,
        _: Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a child constructor binding; defaults to its expression subtree.
    fn argument<'a>(
        &mut self,
        _: view::Argument<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles one completed layout property; defaults to its expression subtree.
    fn layout_property<'a>(
        &mut self,
        _: super::LayoutProperty<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
}
