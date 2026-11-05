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
) -> Result<(), B::Error> {
    super::documentation::emit(node, &ctx, backend)?;
    match node {
        Node::Root(_) => backend.emit_all(ctx.children()),
        Node::MapLayout(v) => backend.map_layout(v, ctx),
        Node::Bank(v) => backend.bank(v, ctx),
        Node::BankProperty(v) => backend.bank_property(v, ctx),
        Node::RegisterPlacement { view, .. } => backend.register_placement(view, ctx),
        Node::Register(v) => backend.register(v, ctx),
        Node::RegisterMap(v) => backend.register_map(v, ctx),
        Node::Placement(v) => backend.placement(v, ctx),
        Node::RegisterField(v) => backend.register_field(v, ctx),
        Node::Parameter(v) => backend.parameter(v, ctx),
        Node::Argument(v) => backend.argument(v, ctx),
        Node::Attribute(v) => match v {
            Attribute::Width(v) => backend.register_width(v, ctx),
            Attribute::Endian(v) => backend.register_endian(v, ctx),
            Attribute::Access(v) => backend.register_access(v, ctx),
            Attribute::Reset { kind, value } => backend.register_reset(kind, value, ctx),
            Attribute::WritableMask(v) => backend.register_writable_mask(v, ctx),
            Attribute::Alignment(v) => match ctx.register() {
                Some(_) => backend.register_alignment(v, ctx),
                None => backend.map_alignment(v, ctx),
            },
            Attribute::ExplicitExtent(v) => backend.map_explicit_extent(v, ctx),
            Attribute::ExplicitOffset(v) => backend.placement_explicit_offset(v, ctx),
            Attribute::Count(v) => backend.placement_count(v, ctx),
            Attribute::Bank(v) => backend.placement_bank(v, ctx),
        },
        Node::LayoutProperty(v) => match v.kind {
            LayoutPropertyKind::Offset => backend.layout_offset(v.expression, ctx),
            LayoutPropertyKind::Count => backend.layout_count(v.expression, ctx),
            LayoutPropertyKind::Stride => backend.layout_stride(v.expression, ctx),
            LayoutPropertyKind::End => backend.layout_end(v.expression, ctx),
            LayoutPropertyKind::OccupiedEnd => backend.layout_occupied_end(v.expression, ctx),
            LayoutPropertyKind::Extent => backend.layout_extent(v.expression, ctx),
        },
        Node::Value(v) => super::super::value::emit(v, ctx, backend),
    }
}
