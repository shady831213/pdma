use hardware_interface_pipeline::{Operation, StepOutcome, Then};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::dma::{
    ChannelRegisters, Command, CommandRegisters, Endpoint, ResponseRegisters,
};
use pdma_model_dma::{
    CheckRanges, ChunkInput, CommandRequest, Dma, Fetch, FetchedCommand, Flow, Pipeline,
    PrepareChunk, ReadCopy, WriteCopy,
};

/// Test harness error propagation.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Assembles complete command execution through response publication.
pub fn chunk_pipeline<M>(device: &Dma<M>) -> Pipeline {
    Then::new(
        pdma_model_dma::CommandExecution::new(Then::new(
            PrepareChunk::default(),
            Then::new(ReadCopy::default(), WriteCopy::default()),
        )),
        Then::new(pdma_model_dma::CompletionInput, device.responses()),
    )
}

/// Fetches a fresh descriptor through the public selected-channel interface.
pub fn fetch<M: MemoryAccess>(
    device: &mut Dma<M>,
    id: ChannelId,
) -> TestResult<Flow<FetchedCommand>> {
    let Some(CommandRequest::Fetch(request)) = device.command_request(id)? else {
        return Err("no fresh command request".into());
    };
    match Operation::new(Some(CommandRequest::Fetch(request))).step(&mut Fetch, device)? {
        StepOutcome::Complete(Ok(command)) if command.data().is_some() => Ok(command),
        StepOutcome::Complete(Err(error)) => Err(error.into()),
        StepOutcome::Complete(Ok(_)) | StepOutcome::InProgress => {
            Err("descriptor was not accepted".into())
        }
    }
}

/// Validates one accepted descriptor and installs its hardware traversal.
pub fn accept<M: MemoryAccess>(device: &mut Dma<M>, id: ChannelId) -> TestResult<Flow<ChunkInput>> {
    let fetched = fetch(device, id)?;
    let mut context = device.execution_context();
    let mut operation = Operation::new(fetched);
    let mut validation = CheckRanges::default();
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut validation, &mut context)? {
            return Ok(context.install_progress(result?));
        }
    }
    Err("validation exceeded interaction bound".into())
}

/// Configure mapped test memory and one enabled channel.
pub fn setup(
    response_base: u64,
    capacity: u32,
) -> Result<(Dma<SharedMemory>, SharedMemory), Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    let region = MemoryRegion::try_new(MemoryAddress::new(0x1000), 0x1000, MemoryAlignment::ONE)?;
    memory.add_regions(&[region])?;
    let device = configure(memory.clone(), response_base, capacity)?;
    Ok((device, memory))
}

/// Configure a channel over an injected test memory capability.
pub fn configure<M: MemoryAccess>(
    memory: M,
    response_base: u64,
    capacity: u32,
) -> Result<Dma<M>, Box<dyn std::error::Error>> {
    let mut device = Dma::new(memory, 1)?;
    for (offset, value) in [
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::COMMAND.offset()
                + CommandRegisters::BASE.offset(),
            0x1000,
        ),
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::BASE.offset(),
            response_base,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CAPACITY.offset(),
            capacity,
        ),
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::CAPACITY.offset(),
            capacity,
        ),
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ_ENABLE.offset(),
            1,
        ),
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    Ok(device)
}

/// Publish the first test descriptor through memory and MMIO.
pub fn submit(
    device: &mut Dma<SharedMemory>,
    memory: &SharedMemory,
    command: Command,
) -> TestResult {
    memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    device.write(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}

/// Drive the hardware pipeline to one published response within a fixed bound.
pub fn run(device: &mut Dma<SharedMemory>) -> TestResult {
    let mut hardware = device.hardware();
    let mut operation = Operation::new(());
    let producer = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
        + pdma_model_contract::dma::ChannelBlock::RESPONSE.offset()
        + ResponseRegisters::PRODUCER.offset();
    let initial = read_word(device, producer)?;
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, device)? {
            result?;
            if read_word(device, producer)? != initial {
                return Ok(());
            }
            operation = Operation::new(());
        }
    }
    Err("pipeline exceeded interaction bound".into())
}

/// Read one exact 32-bit register through the device bus.
pub fn read_word(
    device: &mut Dma<SharedMemory>,
    offset: usize,
) -> Result<u32, Box<dyn std::error::Error>> {
    let mut bytes = [0; 4];
    device.read(offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Canonical four-byte copy input.
pub const fn command() -> Command {
    Command {
        flags: pdma_model_contract::dma::CompletionFlagsValues { ioc: false },
        id: 7,
        ..Command::copy(
            0,
            pdma_model_contract::dma::CopyCommand {
                copy_length: 4,
                sg_context: 0,
                source: Endpoint::linear(0x1200, 0),
                destination: Endpoint::linear(0x1300, 0),
            },
        )
    }
}
