mod error;
mod shape;
use super::AnalysisError;
use crate::ast::{CaseBody, Node, RecordDefinition};
pub use error::ShapeError;

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
        shape::field(field)?;
        node(Node::Record(crate::ast::record::Node::Field(field)))?;
    }
    Ok(())
}

pub(super) fn node(value: Node<'_>) -> Result<(), AnalysisError> {
    match value {
        Node::Register(crate::ast::register::Node::Register(register)) => register
            .validate_shape()
            .map_err(|source| AnalysisError::Register {
                path: register.name(),
                source,
            })?,
        Node::Record(crate::ast::record::Node::Record(value)) => return record(value),
        Node::Record(crate::ast::record::Node::Case(case)) => {
            if let CaseBody::Inline(value) = case.body() {
                value
                    .spec()
                    .resolve()
                    .map_err(|source| AnalysisError::Record {
                        path: case.name(),
                        source,
                    })?;
                return record(value);
            }
        }
        Node::Register(crate::ast::register::Node::RegisterField(field)) => {
            if let Some(enumeration) = field.definition().enum_definition() {
                shape::enumeration_values(field.name(), enumeration, field.bit_width())?;
            }
        }
        Node::Record(crate::ast::record::Node::Union(union)) => shape::union(union)?,
        Node::Record(crate::ast::record::Node::Reference(reference))
        | Node::Register(crate::ast::register::Node::Reference(reference)) => {
            if reference.selector_binding().is_some()
                && !matches!(
                    reference.target().body(),
                    crate::ast::DefinitionBody::Union(_)
                )
            {
                return Err(AnalysisError::SourceShape {
                    path: reference.name(),
                    source: ShapeError::SelectorBinding,
                });
            }
        }
        Node::Value(crate::ast::value::Node::Bits(bits)) => shape::bits("bits", bits)?,
        Node::Record(
            crate::ast::record::Node::Field(_)
            | crate::ast::record::Node::Attribute(_)
            | crate::ast::record::Node::Constant(_)
            | crate::ast::record::Node::Value(_),
        )
        | Node::Register(
            crate::ast::register::Node::RegisterMap(_)
            | crate::ast::register::Node::Placement(_)
            | crate::ast::register::Node::Parameter(_)
            | crate::ast::register::Node::Argument(_)
            | crate::ast::register::Node::Attribute(_)
            | crate::ast::register::Node::LayoutProperty(_)
            | crate::ast::register::Node::Value(_),
        )
        | Node::Value(_) => {}
    }
    for child in value.children() {
        node(child)?;
    }
    Ok(())
}

/// Validates source semantics independently of target language and instance bindings.
pub fn run(graph: &crate::ast::SchemaGraph) -> Result<(), AnalysisError> {
    for definition in graph.dependency_order() {
        node(definition.node())?;
    }
    Ok(())
}
