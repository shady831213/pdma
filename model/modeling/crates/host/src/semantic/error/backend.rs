use std::sync::Arc;
use std::{error::Error, fmt};

/// Transport-independent category for one semantic port backend failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortBackendKind {
    /// Device-visible memory provisioning failed.
    Provisioning,
    /// One register access failed.
    Register,
    /// Checked memory access failed.
    Memory,
}

/// Opaque backend cause exposed without coupling the semantic API to a transport.
#[derive(Debug)]
pub struct PortBackendError {
    kind: PortBackendKind,
    source: Arc<dyn Error + Send + Sync>,
}

impl PortBackendError {
    pub(crate) fn new(kind: PortBackendKind, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind,
            source: Arc::new(source),
        }
    }

    /// Returns the transport-independent failure category.
    pub const fn kind(&self) -> PortBackendKind {
        self.kind
    }
}

impl PartialEq for PortBackendError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for PortBackendError {}

impl fmt::Display for PortBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "host port backend {:?} failed", self.kind)
    }
}

impl Error for PortBackendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
