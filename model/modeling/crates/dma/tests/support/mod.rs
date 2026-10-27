use pdma_model_contract::dma::{
    ChannelConfiguration, ChannelId, ChannelInterrupts, ChannelRegisters, Command,
    CommandRegisters, CopyCommandDestinationValues, CopyCommandSourceValues, EventRegisters,
    LinearEndpoint,
};

use hardware_interface_memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use hardware_interface_pipeline::{Operation, StepOutcome, Then};
use pdma_model_dma::{
    CheckRanges, ChunkInput, CommandRequest, Dma, Fetch, FetchedCommand, Flow, Pipeline,
    PrepareChunk, ReadCopy, WriteCopy,
};

/// Test harness error propagation.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Assembles complete Copy execution with an explicit completion result.
pub fn chunk_pipeline() -> Pipeline {
    pdma_model_dma::CommandExecution::new(Then::new(
        PrepareChunk::default(),
        Then::new(ReadCopy::default(), WriteCopy::default()),
    ))
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
    event_base: u64,
    capacity: u32,
) -> Result<(Dma<SharedMemory>, SharedMemory), Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    let region = MemoryRegion::try_new(MemoryAddress::new(0x1000), 0x1000, MemoryAlignment::ONE)?;
    memory.add_regions(&[region])?;
    let device = configure(memory.clone(), event_base, capacity)?;
    Ok((device, memory))
}

/// Configure a channel over an injected test memory capability.
pub fn configure<M: MemoryAccess>(
    memory: M,
    event_base: u64,
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
                + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
                + EventRegisters::BASE.offset(),
            event_base,
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
                + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
                + EventRegisters::CAPACITY.offset(),
            capacity,
        ),
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ.offset()
                + ChannelInterrupts::ENABLE.offset(),
            1,
        ),
        (
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
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

#[doc = "Drive one finite hardware invocation through actual retirement within a fixed bound."]
pub fn run(device: &mut Dma<SharedMemory>) -> TestResult {
    let mut hardware = device.hardware();
    let mut operation = Operation::new(());
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, device)? {
            return Ok(result?);
        }
    }
    Err("pipeline exceeded interaction bound".into())
}

/// Read one exact 32-bit register through the device bus.
pub fn read_word<M>(device: &mut Dma<M>, offset: usize) -> Result<u32, Box<dyn std::error::Error>> {
    let mut bytes = [0; 4];
    device.read(offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

#[doc = "Checks that successful work did not publish a DMA event."]
pub fn assert_no_event<M>(device: &mut Dma<M>) -> TestResult {
    let producer = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
        + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
        + EventRegisters::PRODUCER.offset();
    let count = read_word(device, producer)?;
    if count == 0 {
        Ok(())
    } else {
        Err(format!("successful command published {count} error events").into())
    }
}

/// Canonical four-byte copy input.
pub const fn command() -> Command {
    Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 7,
        copy_length: 4,
        sg_context: 0,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0x1200,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0x1300,
            offset: 0,
        }),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    })
}

#[doc = "Borrows the Copy payload of a typed test fixture."]
pub fn copy(command: &Command) -> TestResult<&pdma_model_contract::dma::CopyCommand> {
    match command {
        Command::Copy(copy) => Ok(copy),
        Command::Write(_)
        | Command::Fill(_)
        | Command::Wait(_)
        | Command::Atomic(_)
        | Command::Semaphore(_)
        | Command::Fence(_)
        | Command::Trap(_)
        | Command::Timestamp(_) => Err("Copy fixture required".into()),
    }
}

#[doc = "Mutably borrows the Copy payload of a typed test fixture."]
pub fn copy_mut(command: &mut Command) -> TestResult<&mut pdma_model_contract::dma::CopyCommand> {
    match command {
        Command::Copy(copy) => Ok(copy),
        Command::Write(_)
        | Command::Fill(_)
        | Command::Wait(_)
        | Command::Atomic(_)
        | Command::Semaphore(_)
        | Command::Fence(_)
        | Command::Trap(_)
        | Command::Timestamp(_) => Err("Copy fixture required".into()),
    }
}
