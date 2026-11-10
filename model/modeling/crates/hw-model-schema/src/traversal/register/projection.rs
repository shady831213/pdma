use super::{Context, Map, Node};
use crate::traversal::{Edge, Visit};
impl Context<'_> {
    /// Explicit flat-address output from a fully bound declaration. Default
    /// traversal never combines this projection with the declaration tree.
    pub fn projected_children(&self) -> std::vec::IntoIter<Visit<Self>> {
        let nodes = match self.node() {
            Node::RegisterMap(Map::Declaration {
                projection: Some(view),
                ..
            }) => Node::projection_children(view),
            _ => Vec::new(),
        };
        nodes
            .into_iter()
            .enumerate()
            .map(|(i, node)| self.descendant(node, Edge::Projection(i)).current())
            .collect::<Vec<_>>()
            .into_iter()
    }
    /// Explicit concrete instances of a template bank, preserving original indices.
    pub fn instances(&self) -> std::vec::IntoIter<Visit<Self>> {
        let nodes = match self.node() {
            Node::Bank(bank) => bank.instances(),
            _ => Vec::new(),
        };
        nodes
            .into_iter()
            .enumerate()
            .map(|(i, node)| self.descendant(node, Edge::Projection(i)).current())
            .collect::<Vec<_>>()
            .into_iter()
    }
}
