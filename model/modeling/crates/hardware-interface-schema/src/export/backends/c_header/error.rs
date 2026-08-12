use core::fmt;

/// Failure while projecting or emitting a resolved C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CHeaderExportError {
    /// Two resolved paths project to one C11-significant identifier prefix.
    NameCollision {
        /// Earlier resolved path.
        first_path: String,
        /// Later resolved path.
        second_path: String,
        /// Earlier declaration-order position.
        first_index: usize,
        /// Later declaration-order position.
        second_index: usize,
    },
    /// Application sink rejection used by typed sink implementations.
    SinkRejected {
        /// Application-defined rejection code.
        code: u32,
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
            Self::SinkRejected { code } => write!(formatter, "artifact sink rejected code {code}"),
        }
    }
}

impl std::error::Error for CHeaderExportError {}
