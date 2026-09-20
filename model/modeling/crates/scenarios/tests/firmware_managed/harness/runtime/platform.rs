use super::BuildError;
use hardware_interface_schema::RegisterShape;
use pdma_model_contract::{
    ChannelWindowRegisters, ControlRegisters, DmaRegisters, FirmwareRegisters, FunctionId,
};
use pdma_model_hw_components::WindowConfigError;
use pdma_model_software::control::ManagementAddresses;

/// Default placement used only by this scenario platform.
pub const ADDRESSES: ManagementAddresses = ManagementAddresses {
    dma: 0,
    windows: 0x0010_0000,
    control: 0x0011_0000,
    firmware: 0x0012_0000,
};
pub(super) const PF: FunctionId = FunctionId::new(0);
pub(crate) const WINDOW_CONTROL_BASE: u32 = ADDRESSES.windows;
pub(crate) const WINDOW_STATUS_BASE: u32 = ADDRESSES.windows + 8;
pub(crate) const CONTROL_CHANNEL_BASE: u32 = ADDRESSES.control;
pub(crate) const FIRMWARE_STATUS_BASE: u32 = ADDRESSES.firmware;

pub(super) fn validate_functions(
    config: pdma_model_firmware::Configuration,
    count: usize,
) -> Result<(), BuildError> {
    let addresses = config.addresses;
    let invalid = || BuildError::Window(WindowConfigError::AddressUnrepresentable);
    let dma = DmaRegisters::new(count).map_err(|_| invalid())?;
    let windows = ChannelWindowRegisters::new(count).map_err(|_| invalid())?;
    if config.function_count == 0 || u32::try_from(config.function_count).is_err() {
        return Err(invalid());
    }
    let blocks = [
        (addresses.dma, dma.extent(), dma.alignment()),
        (addresses.windows, windows.extent(), windows.alignment()),
        (
            addresses.control,
            ControlRegisters::EXTENT,
            ControlRegisters::ALIGNMENT,
        ),
        (
            addresses.firmware,
            FirmwareRegisters::EXTENT,
            FirmwareRegisters::ALIGNMENT,
        ),
    ];
    for (index, &(base, extent, alignment)) in blocks.iter().enumerate() {
        let start = usize::try_from(base).map_err(|_| invalid())?;
        let end = start.checked_add(extent).ok_or_else(invalid)?;
        if start % alignment != 0 || u32::try_from(end - 1).is_err() {
            return Err(invalid());
        }
        for &(peer, size, _) in blocks.iter().skip(index + 1) {
            let peer = usize::try_from(peer).map_err(|_| invalid())?;
            let peer_end = peer.checked_add(size).ok_or_else(invalid)?;
            if start < peer_end && peer < end {
                return Err(BuildError::Window(
                    WindowConfigError::ReservedAddressOverlap,
                ));
            }
        }
    }
    Ok(())
}
