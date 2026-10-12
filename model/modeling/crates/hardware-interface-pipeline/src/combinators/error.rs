/// Structural plan failures or an error from application code.
#[derive(Debug, PartialEq, Eq)]
pub enum BranchError<E> {
    /// A requested port does not exist.
    InvalidIndex {
        /// Requested port position.
        index: usize,
        /// Number of configured ports.
        ports: usize,
    },
    /// A port appears more than once in one operation.
    DuplicateIndex {
        /// Repeated port position.
        index: usize,
    },
    /// A policy or child stage failed.
    Application(E),
    /// A specialization received an unexpected number of results.
    OutputCount {
        /// Required result count.
        expected: usize,
        /// Received result count.
        actual: usize,
    },
}
