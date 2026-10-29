/// Portable arithmetic supported by parameterized layout exporters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutExpr {
    /// Nonnegative literal.
    Constant(usize),
    /// Named parameter of the enclosing map.
    Parameter(&'static str),
    /// Checked sum.
    Add(Box<Self>, Box<Self>),
    /// Checked difference.
    Subtract(Box<Self>, Box<Self>),
    /// Checked product.
    Multiply(Box<Self>, Box<Self>),
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

impl LayoutExpr {
    /// Evaluates using explicit arguments without executing source-language expressions.
    pub fn evaluate(&self, parameters: &[&str], arguments: &[usize]) -> Result<usize, LayoutError> {
        if parameters.len() != arguments.len() {
            return Err(LayoutError::Arguments);
        }
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Parameter(name) => parameters
                .iter()
                .position(|parameter| parameter == name)
                .and_then(|index| arguments.get(index))
                .copied()
                .ok_or(LayoutError::Parameter(name)),
            Self::Add(left, right) => left
                .evaluate(parameters, arguments)?
                .checked_add(right.evaluate(parameters, arguments)?)
                .ok_or(LayoutError::Geometry),
            Self::Subtract(left, right) => left
                .evaluate(parameters, arguments)?
                .checked_sub(right.evaluate(parameters, arguments)?)
                .ok_or(LayoutError::Geometry),
            Self::Multiply(left, right) => left
                .evaluate(parameters, arguments)?
                .checked_mul(right.evaluate(parameters, arguments)?)
                .ok_or(LayoutError::Geometry),
            Self::Unsupported(source) => Err(LayoutError::Unsupported(source)),
        }
    }
}
