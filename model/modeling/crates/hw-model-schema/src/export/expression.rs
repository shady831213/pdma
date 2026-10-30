use crate::ast::{
    ChildLayout, ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlacementPlan, PlanValue,
};

/// Checked layout properties associated with an original placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutProperty {
    /// First byte offset.
    Offset,
    /// Instance count.
    Count,
    /// Instance spacing.
    Stride,
}
impl LayoutProperty {
    /// Canonical property spelling before target escaping.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Count => "count",
            Self::Stride => "stride",
        }
    }
}

/// A resolved layout operand, independent of target syntax.
#[derive(Debug, Clone, Copy)]
pub enum ExpressionReference<'a> {
    /// Enclosing map parameter.
    Parameter(&'a str),
    /// Previously computed occupied extent.
    Cursor,
    /// Already computed placement property.
    Placement {
        /// Source member name.
        name: &'a str,
        /// Selected property.
        property: LayoutProperty,
    },
    /// Extent returned by a referenced child map.
    ChildExtent(&'a str),
    /// Current placement's exclusive end.
    End(&'a str),
}

/// Nonrecursive target spelling with shared C-like defaults.
pub trait ExpressionSyntax: Sized {
    /// Formats a nonnegative literal.
    fn literal(&mut self, value: usize) -> String {
        value.to_string()
    }
    /// Escapes a source identifier.
    fn identifier(&mut self, value: &str) -> String {
        value.into()
    }
    /// Formats a maximum with folded operands.
    fn maximum(&mut self, left: &str, right: &str) -> String {
        format!("({left} > {right} ? {left} : {right})")
    }
    /// Formats an operator after both children have been folded.
    fn binary(&mut self, op: ExpressionOp, left: &str, right: &str) -> String {
        match op {
            ExpressionOp::Add => format!("({left} + {right})"),
            ExpressionOp::Subtract => format!("({left} - {right})"),
            ExpressionOp::Multiply => format!("({left} * {right})"),
            ExpressionOp::Maximum => self.maximum(left, right),
            ExpressionOp::AlignUp => {
                let one = self.literal(1);
                format!("(({left} + {right} - {one}) & ~({right} - {one}))")
            }
        }
    }
    /// Formats a resolved symbol; override for target-specific access syntax.
    fn reference(&mut self, reference: ExpressionReference<'_>) -> String {
        match reference {
            ExpressionReference::Parameter(name) => format!("p_{}", self.identifier(name)),
            ExpressionReference::Cursor => "cursor".into(),
            ExpressionReference::Placement { name, property } => {
                format!("result.{}_{}", self.identifier(name), property.name())
            }
            ExpressionReference::ChildExtent(name) => {
                format!("child_{}.extent", self.identifier(name))
            }
            ExpressionReference::End(name) => format!("end_{}", self.identifier(name)),
        }
    }
}

/// Formats a symbol through the selected syntax.
pub fn format_reference<S: ExpressionSyntax>(
    reference: ExpressionReference<'_>,
    syntax: &mut S,
) -> String {
    syntax.reference(reference)
}

/// Resolves layout operands and folds an expression through shared recursion.
///
/// # Errors
/// Rejects unsupported expressions and placement operands outside a placement.
pub fn format_expression<S: ExpressionSyntax>(
    expression: &LayoutExpr,
    placement: Option<&PlacementPlan>,
    syntax: &mut S,
) -> Result<String, LayoutError> {
    expression.visit(&mut Format { syntax, placement })
}

struct Format<'a, S> {
    syntax: &'a mut S,
    placement: Option<&'a PlacementPlan>,
}
impl<S: ExpressionSyntax> ExpressionVisitor for Format<'_, S> {
    type Output = String;
    fn constant(&mut self, value: usize) -> Result<String, LayoutError> {
        Ok(self.syntax.literal(value))
    }
    fn parameter(&mut self, name: &'static str) -> Result<String, LayoutError> {
        Ok(self.syntax.reference(ExpressionReference::Parameter(name)))
    }
    fn value(&mut self, value: PlanValue) -> Result<String, LayoutError> {
        if value == PlanValue::Cursor {
            return Ok(self.syntax.reference(ExpressionReference::Cursor));
        }
        let placement = self.placement.ok_or(LayoutError::Geometry)?;
        let name = placement.reference.name();
        let symbol = match value {
            PlanValue::Cursor => ExpressionReference::Cursor,
            PlanValue::ChildAlignment => {
                return self.constant(match &placement.child {
                    ChildLayout::Fixed { alignment, .. } => *alignment,
                    ChildLayout::Call { plan, .. } => plan.alignment(),
                });
            }
            PlanValue::ChildExtent => match &placement.child {
                ChildLayout::Fixed { extent, .. } => return self.constant(*extent),
                ChildLayout::Call { .. } => ExpressionReference::ChildExtent(name),
            },
            PlanValue::Offset => ExpressionReference::Placement {
                name,
                property: LayoutProperty::Offset,
            },
            PlanValue::Count => ExpressionReference::Placement {
                name,
                property: LayoutProperty::Count,
            },
            PlanValue::Stride => ExpressionReference::Placement {
                name,
                property: LayoutProperty::Stride,
            },
            PlanValue::End => ExpressionReference::End(name),
        };
        Ok(self.syntax.reference(symbol))
    }
    fn binary(
        &mut self,
        op: ExpressionOp,
        left: String,
        right: String,
    ) -> Result<String, LayoutError> {
        Ok(self.syntax.binary(op, &left, &right))
    }
}
