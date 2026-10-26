use crate::{RecordSpec, SchemaCodecError, SchemaMemberIdentity};

/// Checks a generated record's own direct alignment and trailing padding.
/// Nested records retain their own padding policy.
#[doc(hidden)]
pub fn validate_record_padding(bytes: &[u8], record: RecordSpec) -> Result<(), SchemaCodecError> {
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte == 0 {
            continue;
        }
        let field = record.fields().iter().any(|field| {
            (field.offset()..field.offset().saturating_add(field.width())).contains(&offset)
        });
        let nested = record.records().iter().any(|record| {
            (0..record.count()).any(|index| {
                let start = record
                    .offset()
                    .saturating_add(index.saturating_mul(record.stride()));
                (start..start.saturating_add(record.definition().size())).contains(&offset)
            })
        });
        if !field && !nested {
            return Err(SchemaCodecError::NonZeroReserved {
                location: SchemaMemberIdentity::new(record.name(), "padding").at_byte(offset),
                observed: u64::from(byte),
            });
        }
    }
    Ok(())
}
