use super::{Definition, SchemaSet};

/// Static metadata producer submitted by schema macros, without model allocation.
#[doc(hidden)]
#[derive(Debug)]
pub struct Registration {
    describe: fn() -> Definition,
}
impl Registration {
    /// Creates a registration for one definition only, without recursive traversal.
    pub const fn new(describe: fn() -> Definition) -> Self {
        Self { describe }
    }
}
inventory::collect!(Registration);

/// Definitions linked into the generator under its actual feature/cfg configuration.
///
/// Discovery does not change Rust visibility or look at source files. Selection
/// includes declared types under a module, including private definitions; export
/// policy and dependency visibility are decisions of the generator.
#[derive(Debug)]
pub struct SchemaRegistry {
    definitions: Vec<Definition>,
}
impl SchemaRegistry {
    /// Reads registered metadata and sorts by defining Rust path.
    pub fn discover() -> Self {
        let mut definitions: Vec<_> = inventory::iter::<Registration>
            .into_iter()
            .map(|registration| (registration.describe)())
            .collect();
        definitions.sort_by_key(|definition| definition.id().path());
        Self { definitions }
    }
    /// Iterates discovered definitions without selecting an export scope.
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.definitions.iter()
    }
    /// Selects definitions below exactly this module path and its descendants.
    pub fn select_module(&self, path: &str) -> SchemaSet {
        let mut set = SchemaSet::new();
        for definition in &self.definitions {
            if definition
                .id()
                .path()
                .strip_prefix(path)
                .is_some_and(|tail| tail.starts_with("::"))
            {
                set.add_definition(definition.clone());
            }
        }
        set
    }
}
