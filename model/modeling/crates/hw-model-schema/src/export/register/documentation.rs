use super::{Backend, Context, Node};
use crate::analysis::Layout;
use crate::export::value::{DocumentationKind as Kind, DocumentationOwner, emit_documentation};

pub(super) fn emit<B: Backend>(
    node: Node<'_>,
    ctx: &Context<'_>,
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
        Node::Register(v) => match ctx.register_location() {
            crate::ast::RegisterLocation::Root => {
                Some((v.documentation(), v.name(), Kind::Register))
            }
            _ => None,
        },
        Node::RegisterField(v) => Some((v.documentation(), v.name(), Kind::BitField)),
        Node::Placement(v) => match &ctx.unit().body {
            Layout::Map(m) if m.instance.is_some() => None,
            Layout::Map(_) | Layout::None | Layout::Record(_) => {
                Some((v.documentation, v.reference.name(), Kind::Placement))
            }
        },
        Node::RegisterPlacement { view, .. } => {
            Some((view.documentation, view.name, Kind::Placement))
        }
        Node::Bank(bank) => Some((bank.documentation(), bank.name(), Kind::Placement)),
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
        Node::BankProperty(_)
        | Node::MapLayout(_)
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
