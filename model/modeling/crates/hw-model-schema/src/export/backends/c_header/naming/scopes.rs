use super::super::CHeaderExportError;
use super::{collision, reject_collisions, significant_prefix};

#[derive(Debug, Default)]
pub(in super::super) struct Scope {
    symbols: crate::export::SymbolScope<(usize, String)>,
    next: usize,
}

impl Scope {
    pub(in super::super) fn insert(
        &mut self,
        name: &str,
        origin: &str,
    ) -> Result<(), CHeaderExportError> {
        self.symbols
            .insert(significant_prefix(name), (self.next, origin.into()))
            .map_err(collision)?;
        self.next += 1;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(in super::super) struct GeometryNames {
    pub(in super::super) members: Scope,
    pub(in super::super) parameters: Scope,
}

impl GeometryNames {
    pub(in super::super) fn new(prefix: &str) -> Result<Self, CHeaderExportError> {
        let type_name = format!("{prefix}_Geometry");
        let function_name = format!("{prefix}_GEOMETRY");
        reject_collisions([
            (type_name.as_str(), type_name.as_str()),
            (function_name.as_str(), function_name.as_str()),
        ])?;
        Ok(Self::default())
    }
}
