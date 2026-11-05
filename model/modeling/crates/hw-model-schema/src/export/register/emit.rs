use super::Node;
use super::{Backend, Context};
use crate::ast::register::{Attribute, LayoutPropertyKind};
/// Dispatches a register node with recursion-local context.
///
/// # Errors
/// Propagates the target error.
pub(super) fn emit<'a, B: Backend>(
    node: Node<'a>,
    ctx: Context<'a>,
    backend: &mut B,
    writer: &mut dyn std::io::Write,
) -> Result<(), B::Error> {
    super::documentation::emit(node, &ctx, backend, writer)?;
    match node {
        Node::Root(_) => backend.emit_all(ctx.children(), writer),
        Node::MapLayout(v) => backend.map_layout(v, ctx, writer),
        Node::Bank(v) => backend.bank(v, ctx, writer),
        Node::BankProperty(v) => backend.bank_property(v, ctx, writer),
        Node::RegisterPlacement { view, .. } => backend.register_placement(view, ctx, writer),
        Node::Register(v) => backend.register(v, ctx, writer),
        Node::RegisterMap(v) => backend.register_map(v, ctx, writer),
        Node::Placement(v) => backend.placement(v, ctx, writer),
        Node::RegisterField(v) => backend.register_field(v, ctx, writer),
        Node::Parameter(v) => backend.parameter(v, ctx, writer),
        Node::Argument(v) => backend.argument(v, ctx, writer),
        Node::Attribute(v) => match v {
            Attribute::Width(v) => backend.register_width(v, ctx, writer),
            Attribute::Endian(v) => backend.register_endian(v, ctx, writer),
            Attribute::Access(v) => backend.register_access(v, ctx, writer),
            Attribute::Reset { kind, value } => backend.register_reset(kind, value, ctx, writer),
            Attribute::WritableMask(v) => backend.register_writable_mask(v, ctx, writer),
            Attribute::Alignment(v) => match ctx.register() {
                Some(_) => backend.register_alignment(v, ctx, writer),
                None => backend.map_alignment(v, ctx, writer),
            },
            Attribute::ExplicitExtent(v) => backend.map_explicit_extent(v, ctx, writer),
            Attribute::ExplicitOffset(v) => backend.placement_explicit_offset(v, ctx, writer),
            Attribute::Count(v) => backend.placement_count(v, ctx, writer),
            Attribute::Bank(v) => backend.placement_bank(v, ctx, writer),
        },
        Node::LayoutProperty(v) => match v.kind {
            LayoutPropertyKind::Offset => backend.layout_offset(v.expression, ctx, writer),
            LayoutPropertyKind::Count => backend.layout_count(v.expression, ctx, writer),
            LayoutPropertyKind::Stride => backend.layout_stride(v.expression, ctx, writer),
            LayoutPropertyKind::End => backend.layout_end(v.expression, ctx, writer),
            LayoutPropertyKind::OccupiedEnd => {
                backend.layout_occupied_end(v.expression, ctx, writer)
            }
            LayoutPropertyKind::Extent => backend.layout_extent(v.expression, ctx, writer),
        },
        Node::Value(v) => super::super::value::emit(v, ctx, backend, writer),
    }
}

pub(super) fn emit_layout_property<B: Backend>(
    backend: &mut B,
    ctx: Context<'_>,
    writer: &mut dyn std::io::Write,
) -> Result<(), B::Error> {
    match ctx.layout_property() {
        Some(property) => backend.layout_property(property, ctx, writer),
        None => backend.emit_all(ctx.children(), writer),
    }
}
