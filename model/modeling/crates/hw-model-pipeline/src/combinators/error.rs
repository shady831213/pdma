/// Invalid value-routing plans or application failures.
#[derive(Debug, PartialEq, Eq)]
pub enum BranchError<E> {
    /// A selected port is outside the configured population.
    InvalidIndex {
        /// Selected position.
        index: usize,
        /// Configured population.
        ports: usize,
    },
    /// A plan selects a port more than once.
    DuplicateIndex {
        /// Repeated position.
        index: usize,
    },
    /// A selected input has no completed value or an output is occupied.
    Unavailable {
        /// Unavailable position.
        index: usize,
    },
    /// A plan tries to produce output without consuming input.
    EmptyInput,
    /// The invocation input population differs from the hardware population.
    InputCount {
        /// Required count.
        expected: usize,
        /// Supplied count.
        actual: usize,
    },
    /// Routed payload count differs from selected destination count.
    OutputCount {
        /// Required count.
        expected: usize,
        /// Supplied count.
        actual: usize,
    },
    /// An application policy or child stage failed.
    Application(E),
}

impl<E: std::fmt::Display> std::fmt::Display for BranchError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIndex { index, ports } => {
                write!(f, "branch index {index} is outside {ports} ports")
            }
            Self::DuplicateIndex { index } => {
                write!(f, "branch index {index} is selected more than once")
            }
            Self::Unavailable { index } => write!(f, "branch port {index} is unavailable"),
            Self::EmptyInput => f.write_str("branch plan produces output without consuming input"),
            Self::InputCount { expected, actual } => {
                write!(f, "branch input count is {actual}, expected {expected}")
            }
            Self::OutputCount { expected, actual } => {
                write!(f, "branch output count is {actual}, expected {expected}")
            }
            Self::Application(error) => write!(f, "branch application failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BranchError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Application(error) => Some(error),
            Self::InvalidIndex { .. }
            | Self::DuplicateIndex { .. }
            | Self::Unavailable { .. }
            | Self::EmptyInput
            | Self::InputCount { .. }
            | Self::OutputCount { .. } => None,
        }
    }
}
