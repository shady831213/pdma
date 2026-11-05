//! Format-neutral streaming export vocabulary.

use core::fmt;
mod generate;
mod symbols;
pub use generate::{Domain, PreparedOutput, generate, prepare, prepare_roots};
pub use symbols::{SymbolCollision, SymbolScope};
/// Shared mathematical expression folding with target-defined spelling.
pub mod expression;
/// Record-only recursive export capability.
pub mod record;
/// Register-only recursive export capability.
pub mod register;
/// Shared value callbacks, independently implemented by each domain target.
pub mod value;
use crate::analysis::AnalysisError;

/// Built-in artifact renderers enabled by format-specific crate features.
#[cfg(any(feature = "export-c-header", feature = "export-veryl-layout"))]
pub mod backends;

/// Format-neutral identity supplied before opening each root output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputMetadata {
    /// Application-defined logical output name with no filesystem meaning.
    pub logical_name: String,
    /// Application-defined media type.
    pub media_type: String,
}

/// Analysis or rendering failure retaining the error owned by its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError<R> {
    /// Invalid analyzed declaration encountered before target emission.
    Analysis(AnalysisError),
    /// The renderer could not complete output.
    Render(R),
}

impl<R: fmt::Display> fmt::Display for ExportError<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(f),
            Self::Render(error) => error.fmt(f),
        }
    }
}

impl<R: std::error::Error + 'static> std::error::Error for ExportError<R> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Analysis(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}
