use crate::Documentation;

use super::RegisterMapPlacement;

/// One nested map instance with its root-qualified path and placement metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRegisterMapPlacement<I> {
    path: String,
    placement: RegisterMapPlacement<I>,
}

impl<I: Copy> ResolvedRegisterMapPlacement<I> {
    /// Associates a resolved path with existing placement metadata.
    pub const fn new(path: String, placement: RegisterMapPlacement<I>) -> Self {
        Self { path, placement }
    }

    /// Returns the root-qualified instance path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the map instance's absolute placement metadata.
    pub const fn placement(&self) -> RegisterMapPlacement<I> {
        self.placement
    }

    /// Returns effective documentation of this map instance.
    pub const fn documentation(&self) -> Documentation {
        self.placement.documentation()
    }
}
