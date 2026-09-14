use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::{
    ChannelRegisters, Command, CommandRegisters, DeviceRegisters, Endpoint, ResponseRegisters,
};
use pdma_model_device::{Device, Pipeline};

/// Test harness error propagation.
pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Configure mapped test memory and one enabled channel.
pub fn setup(
    response_base: u64,
    capacity: u32,
) -> Result<(Device<SharedMemory>, SharedMemory), Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    let region = MemoryRegion::try_new(MemoryAddress::new(0x1000), 0x1000, MemoryAlignment::ONE)?;
    memory.prepare_regions(&[region])?.try_install(&memory)?;
    let mut device = Device::new(memory.clone(), 1)?;
    for (offset, value) in [
        (
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::BASE.offset(),
            0x1000,
        ),
        (
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::BASE.offset(),
            response_base,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CAPACITY.offset(),
            capacity,
        ),
        (
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::CAPACITY.offset(),
            capacity,
        ),
        (
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ_ENABLE.offset(),
            1,
        ),
        (
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    Ok((device, memory))
}

/// Publish the first test descriptor through memory and MMIO.
pub fn submit(
    device: &mut Device<SharedMemory>,
    memory: &SharedMemory,
    command: Command,
) -> TestResult {
    memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    device.write(
        DeviceRegisters::CHANNELS_OFFSET
            + pdma_model_contract::ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}

/// Drive one operation within a fixed interaction bound.
pub fn run(device: &mut Device<SharedMemory>) -> TestResult {
    let mut hardware = device.pipeline();
    let mut operation = Operation::<Pipeline, Device<SharedMemory>>::new(());
    for _ in 0..1024 {
        match StepExecutor::new()
            .step(&mut hardware, &mut operation, device)
            .map_err(|e| format!("{e:?}"))?
        {
            RunOutcome::Progressed => {}
            RunOutcome::Complete(result) => {
                result?;
                return Ok(());
            }
            RunOutcome::Pending(()) => return Err("unexpected stall".into()),
        }
    }
    Err("pipeline exceeded interaction bound".into())
}

/// Read one exact 32-bit register through the device bus.
pub fn read_word(
    device: &mut Device<SharedMemory>,
    offset: usize,
) -> Result<u32, Box<dyn std::error::Error>> {
    let mut bytes = [0; 4];
    device.read(offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Canonical four-byte copy input.
pub const fn command() -> Command {
    Command {
        flags: pdma_model_contract::CompletionFlagsValues { ioc: false },
        id: 7,
        source: Endpoint::linear(0x1200, 4),
        destination: Endpoint::linear(0x1300, 4),
    }
}
