use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Endpoint, EventRegisters,
};
use pdma_model_dma::Dma;
/// Test harness error propagation.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Drives retained work to a terminal result within the test watchdog.
pub fn finish<C, S: Stage<C>>(
    stage: &mut S,
    operation: &mut Operation<S, C>,
    context: &mut C,
) -> TestResult<Result<S::Output, S::Error>> {
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(stage, context)? {
            return Ok(result);
        }
    }
    Err("operation exceeded interaction bound".into())
}

/// Read an exact 32-bit register through the device bus.
pub fn word<M>(device: &mut Dma<M>, offset: usize) -> TestResult<u32> {
    let mut bytes = [0; 4];
    device.read(offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Configure one channel over its independently mapped test region.
pub fn configure<M>(device: &mut Dma<M>, index: usize) -> TestResult {
    let count = usize::try_from(word(
        device,
        pdma_model_contract::dma::DmaRegisters::DISCOVERY.offset()
            + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset(),
    )?)?;
    let base = ChannelId::new(index)
        .register_offset(count)
        .ok_or("channel offset overflow")?;
    let memory = 0x1000 + u64::try_from(index)? * 0x1000;
    for (offset, address) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            memory,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
            memory + 0x100,
        ),
    ] {
        device.write(base + offset, &address.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::IRQ_ENABLE.offset(),
            1,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(base + offset, &value.to_le_bytes())?;
    }
    Ok(())
}

/// Publish a 512-byte Copy followed by its explicit completion Fence.
pub fn submit<M: MemoryAccess>(device: &mut Dma<M>, index: usize, id: u64) -> TestResult
where
    M::Error: std::error::Error + 'static,
{
    let memory = 0x1000 + u64::try_from(index)? * 0x1000;
    let command = Command::copy(
        id,
        pdma_model_contract::dma::CopyCommand {
            copy_length: 512,
            sg_context: 0,
            source: Endpoint::linear(memory + 0x200, 0),
            destination: Endpoint::linear(memory + 0x400, 0),
        },
    );
    device.memory.write(
        MemoryAddress::new(memory),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    publish_fence(device, index, id)
}

/// Append the selected command's Fence before publishing both descriptors.
pub fn publish_fence<M: MemoryAccess>(device: &mut Dma<M>, index: usize, id: u64) -> TestResult
where
    M::Error: std::error::Error + 'static,
{
    let memory = 0x1000 + u64::try_from(index)? * 0x1000;
    let fence = Command::fence(
        id,
        pdma_model_contract::dma::WordWrite {
            target: pdma_model_contract::dma::WordTarget {
                address: memory + 0xf00,
                context: 0,
                width: pdma_model_contract::dma::WordTargetWidth::Bits64,
            },
            value: id.wrapping_add(1),
        },
    );
    device.memory.write(
        MemoryAddress::new(memory + 0xf00),
        MemoryAlignment::ONE,
        &0_u64.to_le_bytes(),
    )?;
    device.memory.write(
        MemoryAddress::new(memory + u64::try_from(Command::RECORD_SIZE)?),
        MemoryAlignment::ONE,
        &fence.to_bytes()?,
    )?;
    let count = usize::try_from(word(
        device,
        pdma_model_contract::dma::DmaRegisters::DISCOVERY.offset()
            + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset(),
    )?)?;
    device.write(
        ChannelId::new(index)
            .register_offset(count)
            .ok_or("channel offset overflow")?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    Ok(())
}

/// Read the channel's software-owned completion word.
pub fn completion<M: MemoryAccess>(device: &Dma<M>, index: usize) -> TestResult<u64>
where
    M::Error: std::error::Error + 'static,
{
    let bytes = device.memory.read_typed::<[u8; 8]>(
        MemoryAddress::new(0x1f00 + u64::try_from(index)? * 0x1000),
        MemoryAlignment::ONE,
    )?;
    Ok(u64::from_le_bytes(bytes))
}
