use super::EmitContext;
use crate::analysis::{
    CompleteMapLayout, CompletePlacementLayout, ConcretePlacement, RegisterMapView,
};
use crate::{
    BitsFieldSpec,
    ast::{
        RegisterInfo,
        register::{Argument, Parameter},
    },
};
/// Target syntax restricted to register source constructs.
pub trait Backend: for<'a> super::super::value::Backend<Context<'a> = EmitContext<'a>> {
    /// Handles a selected register/value root.
    fn root(&mut self, ctx: EmitContext<'_>) -> Result<(), Self::Error> {
        self.emit(ctx.current())
    }
    /// Handles a register declaration.
    fn register<'a>(
        &mut self,
        _value: &'a RegisterInfo,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a source or projected map.
    fn register_map<'a>(
        &mut self,
        _value: &'a RegisterMapView,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a completed symbolic or constant map layout.
    fn map_layout<'a>(
        &mut self,
        _value: &'a CompleteMapLayout,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a concrete placement in a permission view.
    fn concrete_placement<'a>(
        &mut self,
        _value: ConcretePlacement<'a>,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a register bit field.
    fn register_field<'a>(
        &mut self,
        _value: &'a BitsFieldSpec<usize>,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a source or concrete placement.
    fn placement<'a>(
        &mut self,
        _value: &'a CompletePlacementLayout,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a declared constructor parameter and optional root binding.
    fn parameter(&mut self, _: Parameter, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a child constructor binding.
    fn argument<'a>(
        &mut self,
        _value: Argument<'a>,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles the register width attribute.
    fn register_width(&mut self, _: usize, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the register alignment attribute.
    fn register_alignment(&mut self, _: usize, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the register endian attribute.
    fn register_endian(&mut self, _: crate::Endian, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the register access attribute.
    fn register_access(&mut self, _: crate::Access, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the register writable mask attribute.
    fn register_writable_mask(&mut self, _: u64, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the map alignment attribute.
    fn map_alignment(&mut self, _: usize, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the map explicit extent attribute.
    fn map_explicit_extent(&mut self, _: usize, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the placement explicit offset attribute.
    fn placement_explicit_offset(
        &mut self,
        _: usize,
        _: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the placement bank attribute.
    fn placement_bank(&mut self, _: bool, _: EmitContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the register reset classification and wire value.
    fn register_reset(
        &mut self,
        _: crate::ResetKind,
        _: u64,
        _: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles the placement count expression.
    fn placement_count<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles a completed layout property; by default traverses its expression.
    fn layout_property<'a>(
        &mut self,
        _property: super::LayoutProperty<'a>,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles the layout offset expression.
    fn layout_offset<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        match ctx.layout_property() {
            Some(property) => self.layout_property(property, ctx),
            None => self.emit_all(ctx.children()),
        }
    }
    /// Handles the layout count expression.
    fn layout_count<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        match ctx.layout_property() {
            Some(property) => self.layout_property(property, ctx),
            None => self.emit_all(ctx.children()),
        }
    }
    /// Handles the layout stride expression.
    fn layout_stride<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        match ctx.layout_property() {
            Some(property) => self.layout_property(property, ctx),
            None => self.emit_all(ctx.children()),
        }
    }
    /// Handles the layout end expression.
    fn layout_end<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles the layout occupied end expression.
    fn layout_occupied_end<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles the layout extent expression.
    fn layout_extent<'a>(
        &mut self,
        _value: &'a crate::ast::LayoutExpr,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        match ctx.layout_property() {
            Some(property) => self.layout_property(property, ctx),
            None => self.emit_all(ctx.children()),
        }
    }
}
