//! Shallow metadata available to render callbacks; containment remains in bound visits.
mod expression;
mod record;
mod register;
mod storage;
mod value;

pub use expression::{Argument, Expression, LayoutProperty};
pub use record::{Case, Field, Record, Reference, Union};
pub use register::{Bank, Map, MapLayout, Placement, Register, RegisterPlacement};
pub use storage::StorageField;
pub use value::{BitField, Bits, EnumEncoding, Enumeration, Scalar, Storage, StorageMember};

/// Referenced definition identity and scalar facts resolved by shared analysis.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    /// Stable analyzed definition identity.
    pub id: &'a crate::traversal::NodeId,
    /// Declared definition name.
    pub name: &'a str,
    /// Semantic definition category.
    pub kind: crate::export::Kind,
    /// Register access permission, absent for other target categories.
    pub access: Option<crate::Access>,
}
