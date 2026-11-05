//! Independent C11 record codecs and register geometry renderers.
mod analysis;
mod error;
mod fragment;
mod naming;
mod record;
mod register;
mod values;

use crate::export::Artifact;
use crate::traversal::NodeId;
pub use analysis::{CRecordAnalysis, CRecordAnalysisPass};
pub use error::CHeaderExportError;

/// Formats record storage and value declarations as portable C11 codecs.
#[derive(Debug)]
pub struct CRecordBackend<'a> {
    analysis: &'a CRecordAnalysis,
    names: naming::Names,
    artifacts: Vec<Artifact>,
    logical_name: String,
    prefix: String,
    output: fragment::Fragment,
}
impl<'a> CRecordBackend<'a> {
    /// Creates a record renderer borrowing completed C occurrence analysis.
    pub fn new(analysis: &'a CRecordAnalysis) -> Self {
        Self {
            analysis,
            names: naming::Names::default(),
            artifacts: Vec::new(),
            logical_name: String::new(),
            prefix: String::new(),
            output: fragment::Fragment::default(),
        }
    }
    /// Overrides one unit's target prefix.
    #[must_use]
    pub fn with_name(mut self, source: NodeId, target: &str) -> Self {
        self.set_name(source, target);
        self
    }
    /// Overrides one unit's target prefix.
    pub fn set_name(&mut self, source: NodeId, target: &str) {
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
    geometry: register::Geometry,
}
impl CRegisterBackend {
    /// Creates a register renderer using schema names for target symbols.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one unit's target prefix.
    #[must_use]
    pub fn with_name(mut self, source: NodeId, target: &str) -> Self {
        self.set_name(source, target);
        self
    }
    /// Overrides one unit's target prefix.
    pub fn set_name(&mut self, source: NodeId, target: &str) {
        self.names.set(source, target);
    }
}
