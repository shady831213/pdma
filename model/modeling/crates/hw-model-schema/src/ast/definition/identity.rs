use std::any::{TypeId, type_name};

/// Process-local identity plus a deterministic diagnostic/display path.
///
/// Type identity distinguishes versions of the same crate. The Rust path alone
/// is not identity; consumers must diagnose collisions in output names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefinitionId {
    identity: TypeId,
    path: &'static str,
    derived: Vec<String>,
}

impl DefinitionId {
    /// Identifies a Rust definition; reexports retain the same identity.
    pub fn of<T: ?Sized + 'static>() -> Self {
        Self {
            identity: TypeId::of::<T>(),
            path: type_name::<T>(),
            derived: Vec::new(),
        }
    }
    /// Derives identity from a source definition and its structural occurrence path.
    #[must_use]
    pub fn derived(&self, path: &[String]) -> Self {
        let mut id = self.clone();
        id.derived.extend_from_slice(path);
        id
    }
    /// Structural identity suffix; the original source path remains diagnostic provenance.
    pub fn structural_path(&self) -> &[String] {
        &self.derived
    }
    /// Returns the defining Rust type path, not a generated target symbol.
    pub const fn path(&self) -> &'static str {
        self.path
    }
}
