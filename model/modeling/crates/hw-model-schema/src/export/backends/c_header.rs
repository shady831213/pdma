//! C11 whole-node emission with checked storage and map layouts.

mod error;
mod fragment;
mod map;
mod naming;
mod nodes;
mod record;
mod register_map;
mod storage;

use crate::ast::analysis::UnitId;
use crate::export::Artifact;
use std::collections::HashMap;

pub use error::CHeaderExportError;

/// Formats records, bits, registers and concrete or symbolic maps as C11.
#[derive(Debug, Default)]
pub struct CHeaderBackend {
    names: HashMap<UnitId, String>,
    symbols: naming::Scope,
    artifacts: Vec<Artifact>,
    logical_name: String,
    prefix: String,
    output: fragment::Fragment,
    record_root: &'static str,
    map_names: register_map::MapNames,
}

impl CHeaderBackend {
    /// Creates a backend using schema names for target symbols.
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides one analyzed definition's target prefix.
    #[must_use]
    pub fn with_name(mut self, source: impl Into<UnitId>, target: &str) -> Self {
        self.set_name(source, target);
        self
    }

    /// Overrides one analyzed definition's target prefix.
    pub fn set_name(&mut self, source: impl Into<UnitId>, target: &str) {
        self.names.insert(source.into(), target.into());
    }

    fn target_prefix(&self, source: &UnitId, fallback: &str) -> String {
        naming::c_identifier(self.names.get(source).map_or(fallback, String::as_str))
    }
}
