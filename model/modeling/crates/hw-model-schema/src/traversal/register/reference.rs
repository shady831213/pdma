use super::{Context, Node};
use crate::{
    analysis::Layout,
    ast::{DefinitionBody, value},
    traversal::{DomainNode, Edge, Visit},
};
impl Context<'_> {
    /// Expands this type edge in its existing occurrence and output-entry context.
    /// Map geometry was prepared with the use site's arguments; banks stay symbolic.
    pub fn referent(&self) -> Option<Visit<Self>> {
        let unit = self.resolve(self.node().reference()?)?;
        let node = match unit.declaration.as_ref()? {
            DefinitionBody::Enumeration(v) => Node::Value(value::Node::Enum(*v)),
            DefinitionBody::Bits(v) => Node::Value(value::Node::Bits(v)),
            DefinitionBody::Register(v) => Node::Register(v),
            DefinitionBody::Opaque(v) => Node::Value(value::Node::Opaque(v)),
            DefinitionBody::RegisterMap(_) => {
                let layout = self
                    .ancestors()
                    .iter()
                    .rev()
                    .find_map(|n| match n {
                        Node::Placement(p) => p.referent_layout.as_deref(),
                        _ => None,
                    })
                    .or(match &unit.body {
                        Layout::Map(map) => map.plan.as_deref(),
                        _ => None,
                    })?;
                Node::RegisterMap(super::Map::Declaration {
                    layout,
                    projection: None,
                })
            }
            DefinitionBody::Record(_) | DefinitionBody::Union(_) => return None,
        };
        Some(Visit::new(self.descendant(node, Edge::Referent)))
    }
}
