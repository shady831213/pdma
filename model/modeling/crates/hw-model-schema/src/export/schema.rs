//! Target-independent, prepared schemas consumed by all renderers.
mod instance;
mod map;
mod prepare;
use crate::{
    Documentation, ResolvedRecord,
    graph::{BitsDefinition, DefinitionId, MapLayoutPlan, ReferenceKind, RegisterInfo},
};
pub use map::{BankView, ExportPathSegment, RegisterMapView, RegisterView};
pub use prepare::{Bindings, PreparationError};

/// Identity of a reusable definition or an explicitly named export view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExportId {
    /// A Rust schema definition, independent of output naming.
    Definition(DefinitionId),
    /// A generator-owned resolved view.
    View(String),
}
impl From<DefinitionId> for ExportId {
    fn from(id: DefinitionId) -> Self {
        Self::Definition(id)
    }
}
impl From<&str> for ExportId {
    fn from(name: &str) -> Self {
        Self::View(name.into())
    }
}

/// One prepared dependency, retaining the original relationship and wire tag.
#[derive(Debug, Clone)]
pub struct PreparedReference {
    /// Name of the referring member.
    pub name: &'static str,
    /// Relationship to the target definition.
    pub kind: ReferenceKind,
    /// Referenced type identity.
    pub target: DefinitionId,
    /// Union wire discriminant, when present.
    pub discriminant: Option<u64>,
}

/// Complete layout and optional concrete instance of the same register map.
#[derive(Debug, Clone)]
pub struct PreparedMap {
    /// Public formula plan; absent only for supplied resolved permission views.
    pub plan: Option<MapLayoutPlan>,
    /// Concrete metadata when all parameters are bound, or a resolved view is supplied.
    pub instance: Option<RegisterMapView>,
}

/// Prepared schema content; no variant requires backend layout resolution.
#[derive(Debug, Clone)]
pub enum PreparedBody {
    /// Bit fields without an intrinsic byte order.
    Bits(BitsDefinition),
    /// Resolved record, union storage or opaque byte layout.
    Record(ResolvedRecord),
    /// Complete register semantics.
    Register(RegisterInfo),
    /// Symbolic or bound register-map layout.
    Map(PreparedMap),
}

/// Original declaration semantics retained independently of normalized storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    /// Scalar bit layout.
    Bits,
    /// Register definition.
    Register,
    /// Register map or projected map view.
    RegisterMap,
    /// Structured byte record.
    Record,
    /// Union storage and its optional external selector path.
    Union {
        /// External tag path; absent for an untagged union.
        selector: Option<&'static str>,
    },
    /// Application-defined opaque byte layout.
    Opaque,
}

/// A prepared definition and its dependencies, before target naming.
#[derive(Debug, Clone)]
pub struct PreparedDefinition {
    /// Original declaration kind, including union selection semantics.
    pub kind: SchemaKind,
    /// Stable identity within a generation request.
    pub id: ExportId,
    /// Declared name or explicit view name.
    pub name: String,
    /// Definition documentation.
    pub documentation: Documentation,
    /// Original edges, including repeated references.
    pub references: Vec<PreparedReference>,
    /// Complete rendering input.
    pub body: PreparedBody,
}

/// Prepared units in dependency order, independent of the output language.
#[derive(Debug, Clone, Default)]
pub struct PreparedSchema {
    definitions: Vec<PreparedDefinition>,
}
impl PreparedSchema {
    /// Iterates prepared definitions in generation order.
    pub fn definitions(&self) -> impl Iterator<Item = &PreparedDefinition> {
        self.definitions.iter()
    }
    /// Selects one already resolved record under a generator-owned view name.
    pub fn record(name: &str, record: &ResolvedRecord) -> Self {
        Self {
            definitions: vec![PreparedDefinition {
                kind: SchemaKind::Record,
                id: name.into(),
                name: name.into(),
                documentation: record.documentation(),
                references: Vec::new(),
                body: PreparedBody::Record(record.clone()),
            }],
        }
    }
}
