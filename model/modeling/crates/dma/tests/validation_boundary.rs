//! Copy validation rejects misrouted descriptors without stranding their owner.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CommandHeader, CommandRegisters, DmaAdmin, DmaDiscovery,
    Opcode, WaitCommand, WaitCommandComparison, WordTarget, WordTargetWidth,
};

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_dma::{CheckRanges, CommandAdmission, CommandRequest, Dma, Error, Fetch, Retire};
use support::TestResult;

#[test]
fn routing_error_retires_when_fresh_wait_enters_copy_validation() -> TestResult {
    reject_wait(CommandAdmission::Fresh)
}

#[test]
fn routing_error_retires_when_resumed_wait_enters_copy_validation() -> TestResult {
    reject_wait(CommandAdmission::Resume)
}

fn reject_wait(admission: CommandAdmission) -> TestResult {
    // Given: a real Wait descriptor, freshly queued or retained after yielding.
    let (mut dma, memory) = support::setup(0x1100, 2)?;
    let descriptor = Command::Wait(WaitCommand {
        header: CommandHeader {
            opcode: Opcode::Wait,
        },
        id: 41,
        target: WordTarget {
            address: 0x1200,
            context: 0,
            width: WordTargetWidth::Bits32,
        },
        value: 7,
        comparison: WaitCommandComparison::Equal,
    });
    memory.write(0x1200.into(), MemoryAlignment::ONE, b"abcd")?;
    memory.write(0x1300.into(), MemoryAlignment::ONE, b"----")?;
    support::submit(&mut dma, &memory, descriptor)?;
    match admission {
        CommandAdmission::Fresh => {}
        CommandAdmission::Resume => support::run(&mut dma)?,
    }
    let request = dma.command_request(ChannelId::new(0))?;
    assert!(matches!(
        (&request, admission),
        (Some(CommandRequest::Fetch(_)), CommandAdmission::Fresh)
            | (Some(CommandRequest::Resume(_)), CommandAdmission::Resume)
    ));
    let fetched = finish(&mut Fetch, &mut dma, request)?;
    assert_eq!(
        fetched.data().map(|value| *value.command()),
        Some(descriptor)
    );
    assert_eq!(
        fetched
            .data()
            .map(pdma_model_dma::FetchedCommand::admission),
        Some(admission)
    );

    // When: public Copy leaves receive the wrong opcode and propagate to retirement.
    let mut context = dma.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, fetched)?;
    let validation_status = progress.status();
    let validation_has_data = progress.data().is_some();
    let chunk = context.install_progress(progress);
    let installed_status = chunk.status();
    let installed_has_data = chunk.data().is_some();
    let completion = finish(&mut support::chunk_pipeline(), &mut context, chunk);
    let completion = match completion {
        Ok(completion) => completion,
        Err(error) => {
            let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
            dma.write(reset, &1_u32.to_le_bytes())?;
            let reset_busy = support::read_word(&mut dma, reset)?;
            return Err(format!(
                "{admission:?}: validation={validation_status:?}, \
                 validation_has_data={validation_has_data}, installed={installed_status:?}, \
                 installed_has_data={installed_has_data}, graph={error:?}, reset_busy={reset_busy}"
            )
            .into());
        }
    };

    // Then: typed errors retain identity through Then and release accepted/resident work.
    assert_eq!(validation_status, Err(Error::Routing));
    assert!(!validation_has_data);
    assert_eq!(installed_status, Err(Error::Routing));
    assert!(!installed_has_data);
    assert_eq!(completion.status(), Err(Error::Routing));
    assert!(completion.data().is_none());
    assert_eq!(
        Operation::new(vec![(0, completion)]).step(&mut Retire, &mut dma)?,
        StepOutcome::Complete(Err(Error::Routing))
    );
    assert!(dma.command_request(ChannelId::new(0))?.is_none());
    support::assert_no_event(&mut dma)?;
    assert!(!dma.irq());
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1200.into(), MemoryAlignment::ONE)?,
        *b"abcd"
    );
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        *b"----"
    );
    assert_recovered(&mut dma, &memory)
}

fn assert_recovered(dma: &mut Dma<SharedMemory>, memory: &SharedMemory) -> TestResult {
    let successor = support::command();
    memory.write(
        (0x1000 + u64::try_from(Command::RECORD_SIZE)?).into(),
        MemoryAlignment::ONE,
        &successor.to_bytes()?,
    )?;
    dma.write(
        ChannelBlock::ALIGNMENT
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    support::run(dma)?;
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        *b"abcd"
    );
    let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    dma.write(reset, &1_u32.to_le_bytes())?;
    assert_eq!(support::read_word(dma, reset)?, 0);
    Ok(())
}

fn finish<C, S: Stage<C>>(stage: &mut S, context: &mut C, input: S::Input) -> TestResult<S::Output>
where
    S::Error: std::error::Error + 'static,
{
    let mut operation = Operation::new(input);
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(stage, context)? {
            return Ok(result?);
        }
    }
    Err("stage exceeded bounded watchdog".into())
}
