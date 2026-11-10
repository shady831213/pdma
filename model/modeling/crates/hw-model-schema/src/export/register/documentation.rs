use super::{Backend, Context, Node};
use crate::export::value::{DocumentationKind as Kind, DocumentationOwner, emit_documentation};

pub(super) fn emit<B: Backend>(
    node: Node<'_>,
    ctx: &Context<'_>,
    backend: &mut B,
    writer: &mut dyn std::io::Write,
) -> Result<(), B::Error> {
    let docs = match node {
        Node::Root(unit) => match &unit.declaration {
            Some(
                crate::ast::DefinitionBody::RegisterMap(_)
                | crate::ast::DefinitionBody::Bits(_)
                | crate::ast::DefinitionBody::Enumeration(_),
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
        Node::Placement(v) => Some((v.documentation, v.source.name(), Kind::Placement)),
        Node::RegisterPlacement { view, .. } => {
            Some((view.documentation, view.name, Kind::Placement))
        }
        Node::Bank(bank) => Some((bank.documentation(), bank.name(), Kind::Placement)),
        Node::RegisterMap(map) => {
            for (name, documentation) in map
                .projection()
                .filter(|_| map.is_projection())
                .into_iter()
                .flat_map(|map| &map.groups)
            {
                emit_documentation(
                    *documentation,
                    DocumentationOwner {
                        name,
                        kind: Kind::Placement,
                    },
                    ctx,
                    backend,
                    writer,
                )?;
            }
            None
        }
        Node::BankProperty(_)
        | Node::Reference(_)
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
            writer,
        )?;
    }
    Ok(())
}
