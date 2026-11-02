use super::{Backend, EmitContext, Node};
use crate::analysis::{ConcretePlacement, Layout, Placement};
use crate::export::value::{DocumentationKind as Kind, DocumentationOwner, emit_documentation};

pub(super) fn emit<B: Backend>(
    node: Node<'_>,
    ctx: &EmitContext<'_>,
    backend: &mut B,
) -> Result<(), B::Error> {
    let docs = match node {
        Node::Root(unit) => match &unit.declaration {
            Some(
                crate::ast::DefinitionBody::RegisterMap(_) | crate::ast::DefinitionBody::Bits(_),
            )
            | None => Some((unit.documentation, unit.name.as_str(), Kind::Definition)),
            Some(
                crate::ast::DefinitionBody::Register(_)
                | crate::ast::DefinitionBody::Record(_)
                | crate::ast::DefinitionBody::Union(_)
                | crate::ast::DefinitionBody::Opaque(_),
            ) => None,
        },
        Node::Register(v) => match ctx.placement() {
            Some(Placement::Concrete(ConcretePlacement::Register { .. })) => None,
            Some(Placement::Symbolic(_) | Placement::Concrete(ConcretePlacement::Bank { .. }))
            | None => Some((v.documentation(), v.name(), Kind::Register)),
        },
        Node::RegisterField(v) => Some((v.documentation(), v.name(), Kind::BitField)),
        Node::Placement(v) => match &ctx.unit().body {
            Layout::Map(m) if m.instance.is_some() => None,
            Layout::Map(_) | Layout::None | Layout::Record(_) => {
                Some((v.documentation, v.reference.name(), Kind::Placement))
            }
        },
        Node::ConcretePlacement(ConcretePlacement::Register { view, .. }) => {
            Some((view.documentation, view.name, Kind::Placement))
        }
        Node::ConcretePlacement(ConcretePlacement::Bank { view, .. }) => {
            Some((view.documentation, view.name, Kind::Placement))
        }
        Node::RegisterMap(map) => {
            for (name, documentation) in &map.groups {
                emit_documentation(
                    *documentation,
                    DocumentationOwner {
                        name,
                        kind: Kind::Placement,
                    },
                    ctx,
                    backend,
                )?;
            }
            None
        }
        Node::MapLayout(_)
        | Node::Parameter(_)
        | Node::Argument(_)
        | Node::Attribute(_)
        | Node::LayoutProperty(_)
        | Node::Value(_) => None,
    };
    if let Some((documentation, name, kind)) = docs {
        emit_documentation(
            documentation,
            DocumentationOwner { name, kind },
            ctx,
            backend,
        )?;
    }
    Ok(())
}
