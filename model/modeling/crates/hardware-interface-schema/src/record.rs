mod definition;
mod error;
mod ordering;
mod padding;
mod placement;
mod resolve;
mod resolved;
mod spec;
mod validation;
#[doc(hidden)]
pub use padding::validate_record_padding;

pub use definition::{FieldDefinition, FieldSpec};
pub use error::{RecordGraphError, RecordValidationError};
pub use placement::RecordPlacement;
pub use resolved::{ResolvedMember, ResolvedMemberKind, ResolvedRecord};
pub use spec::RecordSpec;
