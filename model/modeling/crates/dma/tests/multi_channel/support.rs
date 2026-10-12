use hardware_interface_pipeline::{AfterEach, Operation, Repeat, Stage, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues,
    Endpoint, ResponseRegisters,
};
use pdma_model_dma::{
    CheckRanges, Dma, Intake, NextChunk, Pipeline, ReadCopy, Resettable, UpdateIrq, WriteCopy,
};
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

/// Assemble one command while keeping arbitration state across test operations.
pub fn single_command<M>(device: &Dma<M>) -> Then<Intake, Pipeline> {
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckRanges, Repeat::new(chunk));
    Then::new(
        device.intake(),
        Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            device.responses(),
        ),
    )
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
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
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
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
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

/// Publish the first 512-byte copy command on a selected channel.
pub fn submit<M: MemoryAccess>(device: &mut Dma<M>, index: usize, id: u64) -> TestResult
where
    M::Error: std::error::Error + 'static,
{
    let memory = 0x1000 + u64::try_from(index)? * 0x1000;
    let command = Command {
        copy_length: 512,
        sg_context: 0,
        id,
        flags: CompletionFlagsValues::default(),
        source: Endpoint::linear(memory + 0x200, 0),
        destination: Endpoint::linear(memory + 0x400, 0),
    };
    device.memory.write(
        MemoryAddress::new(memory),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
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
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}
