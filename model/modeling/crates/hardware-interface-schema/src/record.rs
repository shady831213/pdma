mod definition;
mod error;
mod ordering;
mod placement;
mod resolve;
mod resolved;
mod spec;
mod union;
mod validation;
#[doc(hidden)]
pub use union::validate_union_padding;

pub use definition::{FieldDefinition, FieldSpec};
pub use error::{RecordGraphError, RecordValidationError};
pub use placement::RecordPlacement;
pub use resolved::{ResolvedMember, ResolvedMemberKind, ResolvedRecord};
pub use spec::RecordSpec;
