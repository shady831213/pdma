use crate::{RecordSpec, SchemaCodecError, SchemaMemberIdentity};

/// Checks direct case padding and the inactive tail of a complete-record union.
/// Nested records retain their own padding policy.
#[doc(hidden)]
pub fn validate_union_padding(bytes: &[u8], case: RecordSpec) -> Result<(), SchemaCodecError> {
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte == 0 {
            continue;
        }
        let field = case.fields().iter().any(|field| {
            (field.offset()..field.offset().saturating_add(field.width())).contains(&offset)
        });
        let record = case.records().iter().any(|record| {
            (0..record.count()).any(|index| {
                let start = record
                    .offset()
                    .saturating_add(index.saturating_mul(record.stride()));
                (start..start.saturating_add(record.definition().size())).contains(&offset)
            })
        });
        if !field && !record {
            return Err(SchemaCodecError::NonZeroReserved {
                location: SchemaMemberIdentity::new(case.name(), "padding").at_byte(offset),
                observed: u64::from(byte),
            });
        }
    }
    Ok(())
}
