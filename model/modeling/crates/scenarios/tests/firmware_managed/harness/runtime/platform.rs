use super::{BuildError, PlatformConfigError};
use hardware_interface_schema::RegisterShape;
use pdma_model_contract::dma::DmaRegisters;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::hw_components::firmware::FirmwareRegisters;
use pdma_model_contract::hw_components::windows::ChannelWindowRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::context::{ContextConfigError, ContextTable};
use pdma_model_software::control::ManagementAddresses;

/// Default placement used only by this scenario platform.
pub const ADDRESSES: ManagementAddresses = ManagementAddresses {
    dma: 0,
    windows: 0x0010_0000,
    control: 0x0011_0000,
    firmware: 0x0012_0000,
    contexts: None,
};
pub(super) const PF: FunctionId = FunctionId::new(0);
pub(crate) const WINDOW_CONTROL_BASE: u32 = ADDRESSES.windows;
pub(crate) const WINDOW_STATUS_BASE: u32 = ADDRESSES.windows + 8;
pub(crate) const CONTROL_CHANNEL_BASE: u32 = ADDRESSES.control;
pub(crate) const FIRMWARE_STATUS_BASE: u32 = ADDRESSES.firmware;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ContextPlatform {
    pub(crate) function_count: usize,
    pub(crate) capacity: usize,
}

pub(super) fn contexts(
    capacity: Option<usize>,
    base: Option<u32>,
    channel_count: usize,
) -> Result<Option<std::sync::Arc<std::sync::Mutex<ContextTable>>>, BuildError> {
    capacity
        .zip(base)
        .map(|(count, base)| {
            let base = usize::try_from(base)
                .map_err(|_| BuildError::Platform(PlatformConfigError::AddressSpace))?;
            ContextTable::new(count, channel_count, base)
                .map(|table| std::sync::Arc::new(std::sync::Mutex::new(table)))
                .map_err(BuildError::Context)
        })
        .transpose()
}

pub(super) fn validate_functions(
    config: pdma_model_firmware::Configuration,
    count: usize,
    context_capacity: Option<usize>,
) -> Result<(), BuildError> {
    let addresses = config.addresses;
    let invalid = || BuildError::Platform(PlatformConfigError::AddressSpace);
    let dma = DmaRegisters::new(count)
        .map_err(|error| BuildError::Platform(PlatformConfigError::Layout(error)))?;
    let windows = ChannelWindowRegisters::new(count)
        .map_err(|error| BuildError::Platform(PlatformConfigError::Layout(error)))?;
    if config.function_count == 0 || u32::try_from(config.function_count).is_err() {
        return Err(BuildError::Platform(PlatformConfigError::FunctionCount));
    }
    let mut blocks = vec![
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
    match (addresses.contexts, context_capacity) {
        (Some(base), Some(contexts)) => {
            let table =
                pdma_model_contract::hw_components::context::ContextTableRegisters::for_channels(
                    contexts, count,
                )
                .map_err(|error| BuildError::Context(ContextConfigError::Layout(error)))?;
            blocks.push((base, table.extent(), table.alignment()));
        }
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            return Err(BuildError::Platform(
                PlatformConfigError::ContextConfiguration,
            ));
        }
    }
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
                return Err(BuildError::Platform(PlatformConfigError::Overlap));
            }
        }
    }
    Ok(())
}
#[derive(Clone, Copy, Debug)]
pub(in crate::harness) struct MemoryRequest {
    pub channel: Option<pdma_model_contract::dma::ChannelId>,
    pub identity: pdma_model_contract::memory::MemoryIdentity,
}

pub(in crate::harness) type MemoryBinding<M> = (
    Vec<(pdma_model_contract::memory::MemoryIdentity, M)>,
    Option<fn(&M, MemoryRequest) -> M>,
);
