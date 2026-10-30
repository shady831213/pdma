//! C11 formatting callbacks for the common prepared-schema walker.

mod error;
mod fragment;
mod naming;
mod record;
mod register_map;
mod visit;

use crate::export::{Artifact, ExportId};
use std::collections::HashMap;

pub use error::CHeaderExportError;

/// Formats records, bits, registers and concrete or symbolic maps as C11.
#[derive(Debug, Default)]
pub struct CHeaderRenderer {
    names: HashMap<ExportId, String>,
    symbols: naming::Scope,
    artifacts: Vec<Artifact>,
    logical_name: String,
    prefix: String,
    output: fragment::Fragment,
    record_root: &'static str,
    map_names: register_map::MapNames,
    geometry_names: naming::GeometryNames,
}

impl CHeaderRenderer {
    /// Creates a renderer using schema names for target symbols.
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides one prepared definition's target prefix.
    #[must_use]
    pub fn with_name(mut self, source: impl Into<ExportId>, target: &str) -> Self {
        self.set_name(source, target);
        self
    }

    /// Overrides one prepared definition's target prefix.
    pub fn set_name(&mut self, source: impl Into<ExportId>, target: &str) {
        self.names.insert(source.into(), target.into());
    }

    fn target_prefix(&self, source: &ExportId, fallback: &str) -> String {
        naming::c_identifier(self.names.get(source).map_or(fallback, String::as_str))
    }
}
