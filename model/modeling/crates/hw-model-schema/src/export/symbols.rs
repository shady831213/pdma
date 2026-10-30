use std::collections::{BTreeMap, btree_map::Entry};

/// A target symbol claimed by two source declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolCollision<S> {
    /// Original declaration; failed insertions never replace it.
    pub first: S,
    /// Conflicting declaration.
    pub second: S,
}
impl<S: std::fmt::Debug> std::fmt::Display for SymbolCollision<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "target symbol collision between {:?} and {:?}",
            self.first, self.second
        )
    }
}
impl<S: std::fmt::Debug> std::error::Error for SymbolCollision<S> {}

/// Tracks one target namespace without imposing identifier syntax.
///
/// Backends supply their comparison key: for example the full Veryl symbol or
/// the significant prefix of an already escaped C identifier.
#[derive(Debug)]
pub struct SymbolScope<S> {
    declarations: BTreeMap<String, S>,
}
impl<S> SymbolScope<S> {
    /// Creates an empty namespace.
    pub const fn new() -> Self {
        Self {
            declarations: BTreeMap::new(),
        }
    }
}
impl<S: Clone> SymbolScope<S> {
    /// Claims a comparison key, retaining source information for diagnostics.
    ///
    /// # Errors
    /// Returns both declarations if the key is already claimed, without mutation.
    pub fn insert(&mut self, key: &str, source: S) -> Result<(), SymbolCollision<S>> {
        match self.declarations.entry(key.to_owned()) {
            Entry::Vacant(entry) => {
                entry.insert(source);
                Ok(())
            }
            Entry::Occupied(entry) => Err(SymbolCollision {
                first: entry.get().clone(),
                second: source,
            }),
        }
    }
}
impl<S> Default for SymbolScope<S> {
    fn default() -> Self {
        Self::new()
    }
}
