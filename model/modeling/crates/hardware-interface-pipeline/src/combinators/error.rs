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
