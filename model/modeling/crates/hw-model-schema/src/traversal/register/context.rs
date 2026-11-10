use super::Node;
use crate::analysis::{Layout, ViewPathSegment};
pub use crate::ast::register::LayoutPropertyKind;
use crate::ast::{LayoutExpr, RegisterLocation};
/// Immutable register traversal context bound by the framework.
/// ```compile_fail
/// use hw_model_schema::traversal::register::Context;
/// fn forge(mut ctx: Context<'_>) { ctx.node = ctx.node(); }
/// ```
/// Root construction belongs to the framework:
/// ```compile_fail
/// use hw_model_schema::{analysis::{AnalyzedSchema, Unit}, traversal::register::{Context, Node}};
/// fn root(a: &AnalyzedSchema, u: &Unit) { let _ = Context::root(a, u, Node::Root(u)); }
/// ```
pub type Context<'a> = crate::traversal::BoundContext<'a, Node<'a>>;
impl<'a> Context<'a> {
    /// Enclosing bit-field name.
    pub fn bit(&self) -> Option<&'static str> {
        self.lineage().find_map(|node| match node {
            Node::Value(crate::ast::value::Node::BitField(v)) => Some(v.name()),
            Node::RegisterField(v) => Some(v.name()),
            _ => None,
        })
    }
    /// Enclosing completed bank.
    pub fn bank(&self) -> Option<super::Bank<'a>> {
        self.lineage().find_map(|node| match node {
            Node::Bank(bank) => Some(bank),
            _ => None,
        })
    }
    /// Whether this node is a repeated documentation occurrence.
    pub fn repeated_documentation(&self) -> bool {
        matches!(self.register_location(), RegisterLocation::Instance { .. })
            || self.bank().is_some_and(|bank| bank.expanded)
    }
    /// Enclosing effective register declaration.
    pub fn register(&self) -> Option<&'a crate::ast::RegisterInfo> {
        self.lineage().find_map(|node| match node {
            Node::Register(v) => Some(v),
            _ => None,
        })
    }
}
/// A completed layout property with its semantic owner and source expression.
#[derive(Debug, Clone, Copy)]
pub struct LayoutProperty<'a> {
    /// Relative placement owner; absent for a map-owned property.
    pub owner: Option<&'a str>,
    /// Stable property name independent of target spelling.
    pub name: &'static str,
    /// Completed expression.
    pub expression: &'a LayoutExpr,
    /// Semantic property classification.
    pub kind: LayoutPropertyKind,
}
/// Normalized register position independent of target naming conventions.
#[derive(Debug, Clone, Copy)]
pub struct RegisterSite<'a> {
    /// Root, standalone, template or instance position.
    pub location: RegisterLocation,
    /// Source path.
    pub path: &'a str,
    /// Structural path components, excluding the root.
    pub segments: &'a [ViewPathSegment],
    /// Offset in the enclosing map or bank template.
    pub offset: usize,
}
impl<'a> Context<'a> {
    /// Active unbound map parameter names, regardless of traversal order.
    pub fn parameters(&self) -> &'a [&'static str] {
        self.lineage()
            .find_map(|node| match node {
                Node::RegisterMap(map) => Some(map.parameters()),
                _ => None,
            })
            .unwrap_or_else(|| match &self.definition().body {
                Layout::Map(map) => map.plan.as_ref().map_or(&[], |p| p.parameters()),
                Layout::None | Layout::Record(_) => &[],
            })
    }
    /// Original parameters in the nearest contained map's formal scope.
    pub fn declared_parameters(&self) -> &'a [&'static str] {
        self.lineage()
            .find_map(|node| match node {
                Node::RegisterMap(map) => Some(map.source_parameters()),
                _ => None,
            })
            .unwrap_or_else(|| match &self.definition().body {
                Layout::Map(map) => map.plan.as_ref().map_or(&[], |p| p.source_parameters()),
                Layout::None | Layout::Record(_) => &[],
            })
    }
    /// Concrete bindings in the nearest contained map's formal scope.
    pub fn bindings(&self) -> Option<&'a [usize]> {
        if let Some(map) = self.lineage().find_map(|node| match node {
            Node::RegisterMap(map) => Some(map),
            _ => None,
        }) {
            return map.bindings();
        }
        match &self.definition().body {
            Layout::Map(map) => map.plan.as_ref().and_then(|p| p.bindings()),
            Layout::None | Layout::Record(_) => None,
        }
    }

    /// Normalized position of the enclosing effective register.
    pub fn register_site(&self) -> Option<RegisterSite<'a>> {
        match self
            .lineage()
            .find(|node| matches!(node, Node::RegisterPlacement { .. }))
        {
            Some(Node::RegisterPlacement { view, location }) => Some(RegisterSite {
                location,
                path: &view.path,
                segments: &view.segments,
                offset: view.offset,
            }),
            _ if self
                .lineage()
                .any(|node| matches!(node, Node::Placement(_))) =>
            {
                None
            }
            _ => self.register().map(|_| RegisterSite {
                location: RegisterLocation::Root,
                path: &self.unit().name,
                segments: &[],
                offset: 0,
            }),
        }
    }
    /// Structural register location.
    pub fn register_location(&self) -> RegisterLocation {
        self.register_site()
            .map_or(RegisterLocation::Root, |site| site.location)
    }
    /// Structural register segments, empty at a reusable root.
    pub fn register_segments(&self) -> &'a [ViewPathSegment] {
        self.register_site().map_or(&[], |site| site.segments)
    }
    /// Register source path or selected definition name.
    pub fn semantic_path(&self) -> &'a str {
        self.register_site()
            .map_or_else(|| self.unit().name.as_str(), |site| site.path)
    }
    /// Enclosing completed layout property, with map extent owned by the map itself.
    pub fn layout_property(&self) -> Option<LayoutProperty<'a>> {
        let property = self.lineage().find_map(|node| match node {
            Node::LayoutProperty(property) => Some(property),
            _ => None,
        })?;
        let owner = match self.lineage().find(|node| {
            matches!(
                node,
                Node::Placement(_) | Node::Bank(_) | Node::RegisterMap(_)
            )
        }) {
            Some(Node::Placement(p)) => Some(p.source.name()),
            Some(Node::Bank(bank)) => Some(bank.name()),
            _ => None,
        };
        let name = match property.kind {
            LayoutPropertyKind::Offset => "offset",
            LayoutPropertyKind::Count => "count",
            LayoutPropertyKind::Stride => "stride",
            LayoutPropertyKind::End => "end",
            LayoutPropertyKind::OccupiedEnd => "occupied_end",
            LayoutPropertyKind::Extent => "extent",
        };
        Some(LayoutProperty {
            owner,
            name,
            expression: property.expression,
            kind: property.kind,
        })
    }
}

impl<'a> Context<'a> {
    /// Prepared local opaque geometry at this occurrence or its storage child.
    pub fn opaque_layout(&self) -> Option<&'a crate::ResolvedRecord> {
        let mut current = Some(self.clone());
        while let Some(ctx) = current {
            if matches!(ctx.node(), Node::Value(crate::ast::value::Node::Opaque(_))) {
                let id = ctx
                    .ancestors()
                    .iter()
                    .rev()
                    .find_map(|node| {
                        use crate::traversal::DomainNode;
                        node.reference()
                    })
                    .map_or_else(
                        || ctx.id().clone(),
                        |r| ctx.id().within_referent(r.target().id()),
                    );
                return ctx.analysis().opaque_layout(&id);
            }
            current = ctx.parent();
        }
        None
    }
}
