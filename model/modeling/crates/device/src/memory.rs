use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{AbiBounds, AbiFault, AbiFaultSite, DmaAddress};

pub(crate) fn read(
    memory: &impl MemoryAccess,
    address: DmaAddress,
    length: u64,
) -> Result<Vec<u8>, MemoryAccessError> {
    let address = MemoryAddress::new(address.raw());
    memory.validate(address, MemoryAlignment::ONE, length)?;
    let size = usize::try_from(length)
        .map_err(|_| MemoryAccessError::AddressOverflow { address, length })?;
    let mut bytes = vec![0; size];
    memory.read(address, MemoryAlignment::ONE, &mut bytes)?;
    Ok(bytes)
}

pub(crate) fn write(
    memory: &impl MemoryAccess,
    address: DmaAddress,
    bytes: &[u8],
) -> Result<(), MemoryAccessError> {
    memory.write(
        MemoryAddress::new(address.raw()),
        MemoryAlignment::ONE,
        bytes,
    )
}

pub(crate) const fn fault(error: MemoryAccessError, length: u64, site: AbiFaultSite) -> AbiFault {
    match error {
        MemoryAccessError::AddressOverflow { address, length } => {
            AbiFault::range_overflow_direct(DmaAddress::new(address.raw()), length, site)
        }
        MemoryAccessError::Unmapped { address } | MemoryAccessError::Misaligned { address, .. } => {
            AbiFault::unmapped(DmaAddress::new(address.raw()), length, site)
        }
        MemoryAccessError::CrossesRegion {
            address,
            length,
            region,
        } => match address.checked_add(length) {
            Some(end) => AbiFault::out_of_bounds(
                DmaAddress::new(region.base().raw() + region.length()),
                AbiBounds::new(region.length(), end.raw()),
                site,
            ),
            None => AbiFault::range_overflow_direct(DmaAddress::new(address.raw()), length, site),
        },
    }
}
