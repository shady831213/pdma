use core::fmt;

/// Failure while projecting a resolved C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CHeaderExportError {
    /// Two export origins share one C11-significant identifier prefix.
    NameCollision {
        /// Earlier schema path or emitted symbol.
        first_path: String,
        /// Later schema path or emitted symbol.
        second_path: String,
        /// Earlier position in the validated origin sequence.
        first_index: usize,
        /// Later position in the validated origin sequence.
        second_index: usize,
    },
}

impl fmt::Display for CHeaderExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameCollision {
                first_path,
                second_path,
                first_index,
                second_index,
            } => write!(
                formatter,
                "C identifier collision between {first_path} at {first_index} and {second_path} at {second_index}"
            ),
        }
    }
}

impl std::error::Error for CHeaderExportError {}
