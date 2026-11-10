use super::Node;
use super::{Backend, Context};
use crate::export::{value::Emit as _, view};
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
        Node::Bank(v) => backend.bank(view::Bank(v), ctx, writer),
        Node::BankProperty(v) => backend.bank_property(v, ctx, writer),
        Node::RegisterPlacement { view, .. } => {
            backend.register_placement(view::RegisterPlacement(view), ctx, writer)
        }
        Node::Register(v) => backend.register(view::Register(v), ctx, writer),
        Node::RegisterMap(v) => backend.register_map(view::Map(v), ctx, writer),
        Node::Placement(v) => backend.placement(view::Placement(v), ctx, writer),
        Node::Reference(v) => super::reference::emit(v, ctx, backend, writer),
        Node::RegisterField(v) => {
            backend.register_field(view::BitField::from_spec(v.spec()), ctx, writer)
        }
        Node::Parameter(v) => backend.parameter(v, ctx, writer),
        Node::Argument(v) => backend.argument(view::Argument(v), ctx, writer),
        Node::Attribute(_) => Ok(()),
        Node::LayoutProperty(_) => ctx.layout_property().map_or_else(
            || Ok(()),
            |property| backend.layout_property(property, ctx, writer),
        ),
        Node::Value(v) => {
            let endian = ctx
                .register()
                .map_or(crate::Endian::Little, view::Register::endian);
            super::super::value::emit(v, endian, ctx, backend, writer)
        }
    }
}
