//! Generic C11 fragment exporters.
//!
//! This module exists only with the default-off `export-c-header` feature and remains
//! below [`crate::export`]'s format-neutral extension boundary. Its renderers emit owned,
//! deterministic C11 fragments through an [`crate::export::ArtifactSink`]; callers own
//! files, include guards, and integration. See the crate-level optional-C11 example for
//! the feature declaration and executable `RecordCHeaderExporter` workflow.

mod model;
mod naming;
mod record;
mod register_map;

pub use model::{
    FragmentModelError, RecordCHeaderField, RecordCHeaderModel, RegisterCHeaderBank,
    RegisterCHeaderModel, RegisterMapCHeaderModel,
};
pub use record::RecordCHeaderExporter;
pub use register_map::RegisterMapCHeaderExporter;

const C_MEDIA_TYPE: &str = "text/x-c; charset=utf-8";

#[derive(Debug, Clone, PartialEq, Eq)]
struct FragmentConfig {
    logical_name: String,
    symbol_prefix: String,
}

impl FragmentConfig {
    fn new(logical_name: impl Into<String>, symbol_prefix: impl AsRef<str>) -> Self {
        Self {
            logical_name: logical_name.into(),
            symbol_prefix: naming::c_identifier(symbol_prefix.as_ref()),
        }
    }
}
