use hardware_interface_schema::register_map::RegisterLocation;

use super::{
    MMIO_REGION_SIZE, MmioAccess, MmioAccessError, MmioByteCount, PdmaMmioMap, RegisterSpec,
};

#[derive(Clone, Copy)]
enum Operation {
    Read,
    Write(u64),
}

fn register_for(
    offset: u32,
    width: MmioByteCount,
    operation: Operation,
) -> Result<RegisterSpec, MmioAccessError> {
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
    if u128::from(offset) + width > MMIO_REGION_SIZE {
        return Err(MmioAccessError::UnmappedOffset { offset });
    }
    if u128::from(offset) % width != 0 {
        return Err(MmioAccessError::Misaligned { offset, width });
    }
    let Some(register) = register_at(
        usize::try_from(offset).map_err(|_| MmioAccessError::UnmappedOffset { offset })?,
    ) else {
        return Err(MmioAccessError::ReservedOffset { offset });
    };
    if register.width() != width_usize {
        return Err(MmioAccessError::WrongWidth {
            offset,
            expected: widened(register.width()),
            actual: width,
        });
    }
    match (operation, register.access()) {
        (Operation::Read, MmioAccess::WriteOnly) => {
            Err(MmioAccessError::ReadFromWriteOnly { offset })
        }
        (Operation::Write(_), MmioAccess::ReadOnly) => {
            Err(MmioAccessError::WriteToReadOnly { offset })
        }
        (Operation::Write(value), MmioAccess::ReadWrite | MmioAccess::WriteOnly) => {
            let bits = value & !register.writable_mask();
            if bits == 0 {
                Ok(register)
            } else {
                Err(MmioAccessError::ReservedBits { offset, bits })
            }
        }
        (Operation::Read, MmioAccess::ReadOnly | MmioAccess::ReadWrite) => Ok(register),
    }
}

const fn widened(value: usize) -> MmioByteCount {
    value as MmioByteCount
}

const fn write_width(bytes: &[u8]) -> MmioByteCount {
    bytes.len() as MmioByteCount
}

fn decode_scalar(bytes: &[u8]) -> u64 {
    match bytes.len() {
        4 => <[u8; 4]>::try_from(bytes).map_or(0, |raw| u64::from(u32::from_le_bytes(raw))),
        8 => <[u8; 8]>::try_from(bytes).map_or(0, u64::from_le_bytes),
        _ => 0,
    }
}

/// Returns leaf metadata only at an exact registered MMIO address.
pub fn register_at(offset: usize) -> Option<RegisterSpec> {
    match PdmaMmioMap::locate(offset) {
        Some(RegisterLocation::Fixed { register, .. }) => Some(register),
        Some(RegisterLocation::Repeated { .. }) | None => None,
    }
}

/// Validates one raw read and returns its register metadata.
pub fn decode_read(offset: u32, width: MmioByteCount) -> Result<RegisterSpec, MmioAccessError> {
    register_for(offset, width, Operation::Read)
}

/// Validates and little-endian decodes one raw write.
pub fn decode_write(offset: u32, bytes: &[u8]) -> Result<(RegisterSpec, u64), MmioAccessError> {
    let width = write_width(bytes);
    if width != 4 && width != 8 {
        return Err(MmioAccessError::WrongWidth {
            offset,
            expected: 0,
            actual: width,
        });
    }
    let value = decode_scalar(bytes);
    register_for(offset, width, Operation::Write(value)).map(|register| (register, value))
}
