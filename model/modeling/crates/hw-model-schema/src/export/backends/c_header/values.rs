mod record;
mod register;

use super::{CHeaderExportError, fragment::Fragment, naming};
use crate::export::view::{BitField, Bits};
use std::io::Write;
type CResult<T = ()> = Result<T, CHeaderExportError>;

fn bits_metadata(
    output: &mut Fragment,
    prefix: &str,
    bits: Bits<'_>,
    writer: &mut dyn Write,
) -> CResult {
    output.constants::<64>(prefix, &[("SIZE", bits.scalar().width())], writer)?;
    output.constants::<32>(prefix, &[("BIT_WIDTH", bits.bit_width())], writer)?;
    output.constants::<64>(prefix, &[("DECLARED_MASK", bits.declared_mask())], writer)
}
fn bit_metadata(
    output: &mut Fragment,
    prefix: &str,
    field: BitField<'_>,
    writer: &mut dyn Write,
) -> CResult {
    let prefix = format!("{prefix}_{}", naming::c_identifier(field.name()));
    output.constants::<32>(
        &prefix,
        &[("LSB", field.lsb()), ("BITS", field.bit_width())],
        writer,
    )
}
fn value_prefix(prefix: &str, bit: Option<&str>) -> String {
    bit.map_or_else(
        || prefix.into(),
        |bit| format!("{prefix}_{}", naming::c_identifier(bit)),
    )
}
