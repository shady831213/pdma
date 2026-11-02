//! Independent C11 record codecs and register geometry renderers.
mod error;
mod fragment;
mod naming;
mod record;
mod register;
mod values;

use crate::analysis::UnitId;
use crate::export::Artifact;
pub use error::CHeaderExportError;

/// Formats record storage and value declarations as portable C11 codecs.
#[derive(Debug, Default)]
pub struct CRecordBackend {
    names: naming::Names,
    artifacts: Vec<Artifact>,
    logical_name: String,
    prefix: String,
    output: fragment::Fragment,
}
impl CRecordBackend {
    /// Creates a record renderer using schema names for target symbols.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one unit's target prefix.
    #[must_use]
    pub fn with_name(mut self, source: impl Into<UnitId>, target: &str) -> Self {
        self.set_name(source, target);
        self
    }
    /// Overrides one unit's target prefix.
    pub fn set_name(&mut self, source: impl Into<UnitId>, target: &str) {
        self.names.set(source, target);
    }
}

/// Formats register metadata and map geometry as portable C11.
#[derive(Debug, Default)]
pub struct CRegisterBackend {
    names: naming::Names,
    artifacts: Vec<Artifact>,
    logical_name: String,
    prefix: String,
    output: fragment::Fragment,
    map_names: register::naming::MapNames,
    geometry: register::Geometry,
}
impl CRegisterBackend {
    /// Creates a register renderer using schema names for target symbols.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one unit's target prefix.
    #[must_use]
    pub fn with_name(mut self, source: impl Into<UnitId>, target: &str) -> Self {
        self.set_name(source, target);
        self
    }
    /// Overrides one unit's target prefix.
    pub fn set_name(&mut self, source: impl Into<UnitId>, target: &str) {
        self.names.set(source, target);
    }
}
