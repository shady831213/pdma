use super::AnalysisError;
use crate::ast::{CaseBody, Node, RecordDefinition};

pub(super) fn record(record: &RecordDefinition) -> Result<(), AnalysisError> {
    for (index, field) in record.fields().iter().enumerate() {
        let error = || AnalysisError::SourceField {
            record: record.spec().name(),
            field: field.name(),
        };
        let end = field
            .offset()
            .checked_add(field.width())
            .ok_or_else(error)?;
        if end > record.spec().size() {
            return Err(error());
        }
        for earlier in record.fields().iter().take(index) {
            let earlier_end = earlier
                .offset()
                .checked_add(earlier.width())
                .ok_or_else(error)?;
            if field.width() != 0
                && earlier.width() != 0
                && field.offset() < earlier_end
                && earlier.offset() < end
            {
                return Err(error());
            }
        }
        node(Node::Field(field))?;
    }
    Ok(())
}

pub(super) fn node(value: Node<'_>) -> Result<(), AnalysisError> {
    match value {
        Node::Record(value) => return record(value),
        Node::Case(case) => {
            if let CaseBody::Inline(value) = case.body() {
                return record(value);
            }
        }
        Node::Register(_)
        | Node::RegisterMap(_)
        | Node::Bits(_)
        | Node::Enum(_)
        | Node::Union(_)
        | Node::Field(_)
        | Node::BitField(_)
        | Node::RegisterField(_)
        | Node::Variant(_)
        | Node::Placement(_)
        | Node::Opaque(_) => {}
    }
    for child in value.children() {
        node(child)?;
    }
    Ok(())
}
