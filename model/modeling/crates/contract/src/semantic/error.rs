use core::fmt;

/// Result returned by pure semantic validation.
pub type SemanticResult<T> = Result<T, SemanticError>;

/// Failure owned by model-semantic contract behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticError {
    /// A ring capacity of zero was requested.
    ZeroCapacity,
    /// A producer attempted to push into a full ring.
    RingFull,
}

impl fmt::Display for SemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => formatter.write_str("ring capacity must be greater than zero"),
            Self::RingFull => formatter.write_str("ring is full"),
        }
    }
}

impl std::error::Error for SemanticError {}
