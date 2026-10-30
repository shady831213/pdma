mod evaluate;
mod visitor;
pub use visitor::{ExpressionOp, ExpressionVisitor};

/// Portable arithmetic supported by parameterized layout exporters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutExpr {
    /// Nonnegative literal.
    Constant(usize),
    /// Named parameter of the enclosing map.
    Parameter(&'static str),
    /// A child property or previously computed placement intermediate.
    Value(PlanValue),
    /// Checked sum.
    Add(Box<Self>, Box<Self>),
    /// Checked difference.
    Subtract(Box<Self>, Box<Self>),
    /// Checked product.
    Multiply(Box<Self>, Box<Self>),
    /// Round the first operand up to the second operand's alignment.
    AlignUp(Box<Self>, Box<Self>),
    /// Greater of two values.
    Maximum(Box<Self>, Box<Self>),
    /// Rust expressions outside the portable subset remain visible and are rejected by evaluators.
    Unsupported(&'static str),
}

/// Failure to instantiate or translate a symbolic map definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// Argument count differs from the declared parameter list.
    Arguments,
    /// A referenced parameter has no binding.
    Parameter(&'static str),
    /// Arithmetic overflow, underflow or invalid placement geometry.
    Geometry,
    /// Backend cannot translate an arbitrary Rust expression.
    Unsupported(&'static str),
    /// Two occupied child envelopes overlap.
    Overlap,
    /// A recursive map layout cannot be instantiated by value.
    Cycle(String),
}
impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid schema layout: {self:?}")
    }
}
impl std::error::Error for LayoutError {}

/// Values available while evaluating an ordered placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanValue {
    /// Maximum occupied end before this placement.
    Cursor,
    /// Referenced child's required alignment.
    ChildAlignment,
    /// Referenced child's occupied extent.
    ChildExtent,
    /// Current placement's computed offset.
    Offset,
    /// Current placement's computed count.
    Count,
    /// Current placement's computed stride.
    Stride,
    /// Current placement's computed end.
    End,
}
