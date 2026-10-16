//! Channel requests cross stage boundaries as values, without speculative fetches.

use hardware_interface_pipeline::{Arbitration, Operation, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues,
    Endpoint, ResponseRegisters,
};
use pdma_model_dma::{
    Candidate, CommandRequest, Dma, Error, ExecutionContext, Fetch, FetchedCommand, RoundRobin,
};
use std::cell::RefCell;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct ObservedMemory {
    backing: SharedMemory,
    reads: RefCell<Vec<u64>>,
}

impl MemoryAccess for ObservedMemory {
    type Error = MemoryAccessError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.backing.validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.reads.borrow_mut().push(address.raw());
        self.backing.read(address, alignment, bytes)
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.backing.write(address, alignment, bytes)
    }
}

fn fixture() -> TestResult<Dma<ObservedMemory>> {
    let backing = SharedMemory::new();
    backing.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x2000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(
        ObservedMemory {
            backing,
            reads: RefCell::default(),
        },
        2,
    )?;
    for index in 0..2 {
        let base = ChannelId::new(index).register_offset(2).ok_or("channel")?;
        let address = 0x1000 + u64::try_from(index)? * 0x100;
        let command = Command {
            id: address,
            copy_length: 4,
            flags: CompletionFlagsValues { ioc: false },
            sg_context: 0,
            source: Endpoint::linear(0x2000, 0),
            destination: Endpoint::linear(0x2100, 0),
        };
        device
            .memory
            .backing
            .write(address.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
        for (offset, value) in [
            (
                ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                address,
            ),
            (
                ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
                address + 0x800,
            ),
        ] {
            device.write(base + offset, &value.to_le_bytes())?;
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
                ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
                1,
            ),
            (
                ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
                1,
            ),
        ] {
            device.write(base + offset, &value.to_le_bytes())?;
        }
    }
    Ok(device)
}

#[test]
fn arbitration_hands_only_the_selected_request_to_fetch() -> TestResult {
    // Given: two command rings whose candidate order differs from channel order.
    let mut device = fixture()?;
    let expected_channel = ChannelId::new(1);
    let expected_address = 0x1000 + u64::try_from(expected_channel.index())? * 0x100;
    let mut values = Vec::new();
    for index in [1, 0] {
        let mut source = Candidate::new(ChannelId::new(index));
        let StepOutcome::Complete(Ok(value)) = Operation::new(()).step(&mut source, &mut device)?
        else {
            return Err("candidate did not complete".into());
        };
        values.push(Some(value));
    }
    // When: arbitration selects a completed request, without driving any channel.
    let winner = RoundRobin::default().select(&values)?.ok_or("winner")?;
    let request = values
        .get(winner)
        .copied()
        .flatten()
        .flatten()
        .ok_or("request")?;
    assert!(device.memory.reads.borrow().is_empty());
    let result = Operation::new(match request {
        CommandRequest::Fetch(snapshot) => Some(snapshot),
        CommandRequest::Resume(_) | CommandRequest::Continue(_) => {
            return Err("unexpected continuation".into());
        }
    })
    .step(&mut Fetch, &mut device.execution_context(expected_channel)?)?;
    // Then: only the selected descriptor is read and its identity crosses the boundary.
    let StepOutcome::Complete(Ok(Some(task))) = result else {
        return Err("fetch did not complete".into());
    };
    assert_eq!(request.channel(), expected_channel);
    assert_eq!(task.channel(), expected_channel);
    assert_eq!(device.memory.reads.borrow().as_slice(), &[expected_address]);
    Ok(())
}

struct WorkChannel;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for WorkChannel {
    type Input = Option<FetchedCommand>;
    type Output = Option<ChannelId>;
    type Error = Error;

    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Error> {
        Ok(input.take().map_or(StepOutcome::InProgress, |task| {
            StepOutcome::Complete(Ok(task.as_ref().map(FetchedCommand::channel)))
        }))
    }
}

#[test]
fn then_transfers_fetched_command_to_downstream_without_an_engine_driver() -> TestResult {
    // Given: independent Fetch and command transform connected only by Then.
    let mut device = fixture()?;
    let selected = ChannelId::new(1);
    let expected = 0x1000 + u64::try_from(selected.index())? * 0x100;
    let mut pipeline = Then::new(Fetch, WorkChannel);
    let Some(CommandRequest::Fetch(snapshot)) = device.command_request(selected)? else {
        return Err("fresh snapshot".into());
    };
    let mut operation = Operation::new(Some(snapshot));
    let mut context = device.execution_context(selected)?;
    // When: each side of the connection is advanced through the composition.
    assert_eq!(
        operation.step(&mut pipeline, &mut context)?,
        StepOutcome::InProgress
    );
    let result = operation.step(&mut pipeline, &mut context)?;
    // Then: downstream receives the fetched command without any other descriptor read.
    assert_eq!(result, StepOutcome::Complete(Ok(Some(selected))));
    assert_eq!(device.memory.reads.borrow().as_slice(), &[expected]);
    Ok(())
}

mod request_selection;
