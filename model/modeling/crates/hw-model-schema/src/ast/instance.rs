use super::{DefinitionId, MapGeometry};

/// One named configuration of a map definition, without any hardware state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapInstance {
    pub(super) name: String,
    pub(super) definition: DefinitionId,
    pub(super) arguments: Vec<usize>,
    pub(super) geometry: MapGeometry,
}
impl MapInstance {
    /// Application-selected instance name, independent of the Rust type path.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Reusable map definition identity.
    pub fn definition(&self) -> DefinitionId {
        self.definition.clone()
    }
    /// Arguments in the map's declared parameter order.
    pub fn arguments(&self) -> &[usize] {
        &self.arguments
    }
    /// Checked layout of this particular instance.
    pub const fn geometry(&self) -> &MapGeometry {
        &self.geometry
    }
}
