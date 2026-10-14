use crate::harness::runtime::memory::RoutedMemory;
use hardware_interface_pipeline::{HardwareDriver, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Endpoint, Response,
    ResponseRegisters,
};
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_dma::{Dma, Service};
use pdma_model_hw_components::windows::ChannelWindows;
use std::sync::{Arc, Mutex};

/// Boundary-test error propagation.
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// DMA wired to Function-selected memory capabilities.
pub(super) type Hardware = Dma<RoutedMemory<SharedMemory>, pdma_model_contract::dma::DmaAccess>;
/// Finite DMA service over the fixture's Function-selected memory.
pub(super) type Pipeline = Service;
/// Permission-bank placement in these hardware fixtures.
pub(super) const WINDOWS: usize = 0x10_0000;

pub(super) struct Fixture {
    pub(super) dma: Hardware,
    pub(super) spaces: [SharedMemory; 2],
    pub(super) windows: Arc<Mutex<ChannelWindows>>,
}

/// Creates two independent address spaces and their unenabled channels.
pub(super) fn setup() -> TestResult<Fixture> {
    let spaces = [SharedMemory::new(), SharedMemory::new()];
    for memory in &spaces {
        memory.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x2000,
            MemoryAlignment::ONE,
        )?])?;
    }
    let mut windows = ChannelWindows::new(2, 0, u32::try_from(WINDOWS)?)?;
    windows.management_write(
        WINDOWS
            + ChannelId::new(1).window_control_offset().ok_or("window")?
            + WindowControl::OWNER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let windows = Arc::new(Mutex::new(windows));
    let mut dma = Dma::with_sideband(
        RoutedMemory::new(Arc::clone(&windows), spaces.to_vec())?,
        2,
        |access| access,
    )?;
    configure(&mut dma, ChannelId::new(0))?;
    configure(&mut dma, ChannelId::new(1))?;
    Ok(Fixture {
        dma,
        spaces,
        windows,
    })
}

/// Programs ring geometry before channel enable.
pub(super) fn configure(dma: &mut Hardware, id: ChannelId) -> TestResult {
    let base = id.register_offset(2).ok_or("channel")?;
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
            0x1100_u64,
        ),
    ] {
        dma.write(base + offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
            2_u32,
        ),
    ] {
        dma.write(base + offset, &value.to_le_bytes())?;
    }
    Ok(())
}

/// Enables a configured channel and publishes its first command.
pub(super) fn publish(
    dma: &mut Hardware,
    memory: &SharedMemory,
    (id, command): (ChannelId, Command),
) -> TestResult {
    memory.write_typed(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    dma.write(
        id.register_offset(2).ok_or("channel")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )?;
    dma.write(
        id.register_offset(2).ok_or("channel")?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}

/// Drives retained hardware work within a bounded interaction budget.
pub(super) fn advance(
    dma: &mut Hardware,
    pipeline: &mut Pipeline,
    driver: &mut HardwareDriver<Pipeline, Hardware>,
) -> TestResult {
    for _ in 0..256 {
        match driver
            .step(pipeline, dma)
            .map_err(|error| format!("{error:?}"))?
        {
            StepOutcome::InProgress => {}
            StepOutcome::Complete(result) => result?,
        }
    }
    Ok(())
}

/// Decodes the first software-visible completion.
pub(super) fn response(memory: &SharedMemory) -> TestResult<Response> {
    let bytes = memory.read_typed::<pdma_model_contract::dma::ResponseRaw>(
        MemoryAddress::new(0x1100),
        MemoryAlignment::ONE,
    )?;
    Ok(Response::from_bytes(bytes.as_ref())?)
}

/// Reads an exact output or sentinel extent.
pub(super) fn bytes<const N: usize>(memory: &SharedMemory, address: u64) -> TestResult<[u8; N]> {
    let mut bytes = [0; N];
    memory.read(
        MemoryAddress::new(address),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    Ok(bytes)
}

/// Constructs the fixture's linear copy descriptor.
pub(super) const fn command(id: u64) -> Command {
    Command {
        copy_length: 8,
        sg_context: 0,
        id,
        source: Endpoint::linear(0x1200, 0),
        destination: Endpoint::linear(0x1300, 0),
        flags: pdma_model_contract::dma::CompletionFlagsValues { ioc: false },
    }
}
