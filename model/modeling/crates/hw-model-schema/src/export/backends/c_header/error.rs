use core::fmt;

/// Failure rendering a C fragment or selected definition graph.
#[derive(Debug)]
pub enum CHeaderExportError {
    /// Writing the C output failed.
    Io(std::io::Error),
    /// A mathematical expression is not complete.
    Layout(crate::ast::LayoutError),
    /// A callback refers to unavailable target naming metadata.
    InvalidVisit(&'static str),
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
            Self::Io(error) => error.fmt(formatter),
            Self::Layout(error) => error.fmt(formatter),
            Self::InvalidVisit(context) => {
                write!(formatter, "invalid C formatting visit: {context}")
            }
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

impl std::error::Error for CHeaderExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Layout(source) => Some(source),
            Self::InvalidVisit(_) | Self::NameCollision { .. } => None,
        }
    }
}

impl From<crate::ast::LayoutError> for CHeaderExportError {
    fn from(error: crate::ast::LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<std::io::Error> for CHeaderExportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
