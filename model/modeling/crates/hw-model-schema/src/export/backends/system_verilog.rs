//! SystemVerilog byte records and ordinary register layout packages (no UVM).
mod record;
mod register;
use super::sv::{Names, Symbols};
use crate::traversal::NodeId;

/// Emits byte-image typedefs, field accessors and enum/bit metadata into a caller's package.
///
/// One backend instance owns one target package namespace. Create a new instance for a
/// different package; retrying an entry replaces its previous symbol claims.
#[derive(Debug, Default)]
pub struct SvRecordBackend {
    names: Names,
    symbols: Symbols,
}
impl SvRecordBackend {
    /// Uses source type names without adding package or file policy.
    pub fn new() -> Self {
        Self::default()
    }
    /// Selects the target name for a source definition.
    #[must_use]
    pub fn with_name(mut self, id: NodeId, name: &str) -> Self {
        self.names.set(id, name);
        self
    }
}
/// Emits one `SystemVerilog` layout package per register/map entry.
/// Symbolic layout functions take 64-bit unsigned values, matching analyzed arithmetic.
#[derive(Debug, Default)]
pub struct SvRegisterBackend {
    output: register::Output,
}
impl SvRegisterBackend {
    /// Uses source names followed by `Layout` for package names.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides a package name without changing source identities.
    #[must_use]
    pub fn with_name(mut self, id: NodeId, name: &str) -> Self {
        self.output.names.set(id, name);
        self
    }
}
