use hardware_interface_schema::register::Access;

use super::{MmioAccessError, MmioByteCount, RegisterLocation, locate};

#[derive(Clone, Copy)]
enum Operation {
    Read,
    Write(u64),
}

fn register_for(
    offset: u32,
    width: MmioByteCount,
    operation: Operation,
) -> Result<RegisterLocation, MmioAccessError> {
    let width_usize = match width {
        4 => 4,
        8 => 8,
        _ => {
            return Err(MmioAccessError::WrongWidth {
                offset,
                expected: 0,
                actual: width,
            });
        }
    };
    if u128::from(offset) + width > u128::from(u32::MAX) + 1 {
        return Err(MmioAccessError::UnmappedOffset { offset });
    }
    if u128::from(offset) % width != 0 {
        return Err(MmioAccessError::Misaligned { offset, width });
    }
    let location = locate(offset).ok_or(MmioAccessError::ReservedOffset { offset })?;
    let definition = match location {
        RegisterLocation::Global { register } => register.definition(),
        RegisterLocation::Queue { register, .. } => register.definition(),
        RegisterLocation::Irq { register, .. } => register.definition(),
    };
    if definition.width() != width_usize {
        return Err(MmioAccessError::WrongWidth {
            offset,
            expected: definition.width() as MmioByteCount,
            actual: width,
        });
    }
    match (operation, definition.access()) {
        (Operation::Read, Access::WriteOnly) => Err(MmioAccessError::ReadFromWriteOnly { offset }),
        (Operation::Write(_), Access::ReadOnly) => Err(MmioAccessError::WriteToReadOnly { offset }),
        (Operation::Write(value), Access::ReadWrite | Access::WriteOnly) => {
            let bits = value & !definition.writable_mask();
            if bits == 0 {
                Ok(location)
            } else {
                Err(MmioAccessError::ReservedBits { offset, bits })
            }
        }
        (Operation::Read, Access::ReadOnly | Access::ReadWrite) => Ok(location),
    }
}

/// Validates one raw read against generated register permissions and geometry.
///
/// # Errors
/// Rejects malformed widths, aperture overflow, alignment, holes, and write-only reads.
pub fn decode_read(offset: u32, width: MmioByteCount) -> Result<RegisterLocation, MmioAccessError> {
    register_for(offset, width, Operation::Read)
}

/// Validates and little-endian decodes one complete raw register write.
///
/// # Errors
/// Rejects malformed geometry, read-only writes, and reserved writable bits.
pub fn decode_write(
    offset: u32,
    payload: &[u8],
) -> Result<(RegisterLocation, u64), MmioAccessError> {
    let value = match *payload {
        [byte0, byte1, byte2, byte3] => u64::from(u32::from_le_bytes([byte0, byte1, byte2, byte3])),
        [byte0, byte1, byte2, byte3, byte4, byte5, byte6, byte7] => {
            u64::from_le_bytes([byte0, byte1, byte2, byte3, byte4, byte5, byte6, byte7])
        }
        _ => {
            return Err(MmioAccessError::WrongWidth {
                offset,
                expected: 0,
                actual: payload.len() as MmioByteCount,
            });
        }
    };
    let location = register_for(
        offset,
        payload.len() as MmioByteCount,
        Operation::Write(value),
    )?;
    Ok((location, value))
}
