//! UVM register classes from source maps, without runtime, bus or predictor ownership.
mod build;
mod fields;
mod registers;
mod values;
use super::sv::{Names, Symbols};
use crate::traversal::NodeId;

/// Generates little-endian RAL classes; package/import/file assembly belongs to the caller.
///
/// Source maps and uniform field access are supported. Permission-only projections and
/// partial writable fields are rejected rather than inferring missing hardware policy.
#[derive(Debug)]
pub struct UvmRegisterBackend {
    names: Names,
    symbols: Symbols,
    // Deferred method text, emitted after the class member declarations.
    build_body: Vec<u8>,
    volatile: bool,
    compare: bool,
}
impl Default for UvmRegisterBackend {
    fn default() -> Self {
        Self {
            names: Names::default(),
            symbols: Symbols::default(),
            build_body: Vec::new(),
            volatile: true,
            compare: false,
        }
    }
}
impl UvmRegisterBackend {
    /// Uses conservative volatile/no-mirror-comparison defaults; `PostReset` remains unknown.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides a generated class name, including all references to that class.
    #[must_use]
    pub fn with_name(mut self, id: NodeId, name: &str) -> Self {
        self.names.set(id, name);
        self
    }
    /// Sets the generated RAL field volatility; this does not model hardware effects.
    #[must_use]
    pub const fn with_volatile(mut self, volatile: bool) -> Self {
        self.volatile = volatile;
        self
    }
    /// Selects mirror comparison; the caller must supply any required prediction mechanism.
    #[must_use]
    pub const fn with_mirror_comparison(mut self, enabled: bool) -> Self {
        self.compare = enabled;
        self
    }
}
