use super::Context;
use crate::ast::register::Parameter;
use crate::export::{value::Emit as _, view};
/// Target syntax restricted to register source constructs.
pub trait Backend: for<'a> super::super::value::Backend<Context<'a> = Context<'a>> {
    /// Handles a placement reference and its definition-scoped target visit.
    /// The context retains the use site; the target retains its own formal parameters.
    /// Defaults to no output; explicitly emit the target to inline its definition.
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        _: view::Target<'a>,
        _: crate::export::Visit<Context<'a>>,
        _: Context<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a selected register/value root.
    fn root(
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
    /// Handles a completed symbolic or constant map layout.
    fn map_layout<'a>(
        &mut self,
        _value: view::MapLayout<'a>,
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
    /// Handles a register bit field.
    fn register_field<'a>(
        &mut self,
        _value: view::BitField<'a, usize>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
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
    /// Handles a declared constructor parameter and optional root binding.
    fn parameter(
        &mut self,
        _: Parameter,
        _: Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a child constructor binding; defaults to no output.
    fn argument<'a>(
        &mut self,
        _: view::Argument<'a>,
        _: Context<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles one completed layout property; defaults to no output.
    fn layout_property<'a>(
        &mut self,
        _: super::LayoutProperty<'a>,
        _: Context<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
