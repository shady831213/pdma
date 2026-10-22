use super::{TestResult, configure_channel, queued_channel};
use crate::{Completion, Dma, Error, Fetch, Flow, Retire};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{
    ChannelId, Command, WaitCommand, WaitCommandComparison, WordTarget, WordTargetWidth,
};

fn fetched(
    dma: &mut Dma<SharedMemory>,
    channel: ChannelId,
) -> TestResult<Flow<crate::FetchedCommand>> {
    let request = dma.command_request(channel)?;
    match Operation::new(request).step(&mut Fetch, dma)? {
        StepOutcome::Complete(result) => Ok(result?),
        StepOutcome::InProgress => Err("fetch required".into()),
    }
}

fn waiting(dma: &mut Dma<SharedMemory>) -> TestResult<Flow<Completion>> {
    let command = Command::wait(
        23,
        WaitCommand {
            target: WordTarget {
                address: 0x1200,
                context: 0,
                width: WordTargetWidth::Bits32,
            },
            value: 0,
            comparison: WaitCommandComparison::Equal,
        },
    );
    dma.memory
        .write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
    let flow = fetched(dma, ChannelId::new(0))?;
    let mut stage = crate::stages::operations::Execute::<WaitCommand>::default();
    match Operation::new(flow).step(&mut stage, &mut dma.execution_context())? {
        StepOutcome::Complete(result) => Ok(result?),
        StepOutcome::InProgress => Err("wait must yield".into()),
    }
}

#[test]
fn early_error_retires_later_residents_and_preserves_first_error() -> TestResult {
    // Given: a yielded Wait still owns residency while a peer is accepted.
    let mut dma = queued_channel(ChannelId::new(0))?;
    configure_channel(&mut dma, ChannelId::new(1))?;
    let first = waiting(&mut dma)?.stop(Error::Routing);
    let later = fetched(&mut dma, ChannelId::new(1))?.stop(Error::Halted);
    dma.request_reset();
    assert!(dma.resetting());
    let mut input = Some(vec![(7, first), (3, later)]);
    // When: a collection contains distinct errors in input order.
    let result = Retire.step(&mut dma, &mut input)?;
    // Then: later cleanup completes reset and the first error wins.
    assert_eq!(result, StepOutcome::Complete(Err(Error::Routing)));
    assert!(input.is_none());
    assert!(!dma.resetting());
    assert!(
        dma.channels
            .iter()
            .all(|channel| channel.resident.is_none())
    );
    assert!(dma.channels.iter().all(|channel| channel.work.is_none()));
    assert!(dma.active.is_none());
    Ok(())
}

#[test]
fn empty_collection_completes_but_absent_input_waits() -> TestResult {
    // Given: an idle DMA with no offered result collection.
    let mut dma = queued_channel(ChannelId::new(0))?;
    // When: retirement polls absence, then accepts an empty completed collection.
    assert_eq!(Retire.step(&mut dma, &mut None)?, StepOutcome::InProgress);
    let mut empty = Some(Vec::new());
    // Then: the empty collection completes and is consumed.
    assert_eq!(
        Retire.step(&mut dma, &mut empty)?,
        StepOutcome::Complete(Ok(()))
    );
    assert!(empty.is_none());
    Ok(())
}

#[test]
fn unowned_bubble_completes_without_retiring_resident_work() -> TestResult {
    // Given: a real accepted command and a Fetch bubble without ownership.
    let mut dma = queued_channel(ChannelId::new(1))?;
    let accepted = fetched(&mut dma, ChannelId::new(1))?;
    let StepOutcome::Complete(Ok(bubble)) = Operation::new(None).step(&mut Fetch, &mut dma)? else {
        return Err("fetch bubble required".into());
    };
    // When: retirement consumes only the bubble.
    let result = Retire.step(&mut dma, &mut Some(vec![(8, bubble.finish())]))?;
    // Then: the real owner remains live.
    assert_eq!(result, StepOutcome::Complete(Ok(())));
    assert_eq!(
        dma.channels.get(1).ok_or("channel")?.resident,
        accepted.work
    );
    Ok(())
}

#[test]
fn branch_index_does_not_select_the_completion_channel() -> TestResult {
    // Given: a genuine accepted channel-one flow completed by a custom tail.
    let mut dma = queued_channel(ChannelId::new(1))?;
    let completion = fetched(&mut dma, ChannelId::new(1))?.finish();
    // When: the receipt carries opcode branch zero rather than channel one.
    let result = Retire.step(&mut dma, &mut Some(vec![(0, completion)]))?;
    // Then: its own resident and accepted ownership are released.
    assert_eq!(result, StepOutcome::Complete(Ok(())));
    let channel = dma.channels.get(1).ok_or("channel")?;
    assert!(channel.resident.is_none());
    assert!(channel.work.is_none());
    assert!(dma.active.is_none());
    Ok(())
}

#[test]
fn yielded_wait_retains_accepted_work_after_collection_retirement() -> TestResult {
    // Given: a false Wait has yielded the shared grant without faulting.
    let mut dma = queued_channel(ChannelId::new(0))?;
    let flow = waiting(&mut dma)?;
    assert!(flow.data().is_none());
    // When: its receipt reaches the common collection boundary.
    let result = Retire.step(&mut dma, &mut Some(vec![(3, flow)]))?;
    // Then: accepted ownership survives while residency and grant are released.
    assert_eq!(result, StepOutcome::Complete(Ok(())));
    let channel = dma.channels.first().ok_or("channel")?;
    assert!(channel.resident.is_none());
    assert!(channel.work.is_some());
    assert!(dma.active.is_none());
    assert!(matches!(
        dma.command_request(ChannelId::new(0))?,
        Some(crate::CommandRequest::Resume(_))
    ));
    Ok(())
}

#[test]
fn stale_generation_cannot_clear_replacement_residency() -> TestResult {
    // Given: real accepted receipts at the same channel in different generations.
    let id = ChannelId::new(1);
    let mut original = queued_channel(id)?;
    let stale = fetched(&mut original, id)?.finish();
    let mut replacement = queued_channel(id)?;
    replacement.request_reset();
    configure_channel(&mut replacement, id)?;
    let current = fetched(&mut replacement, id)?;
    // When: an old generation's receipt is misrouted into the replacement context.
    let result = Retire.step(&mut replacement, &mut Some(vec![(0, stale)]))?;
    // Then: generation rejection leaves the current resident, accepted work and grant intact.
    assert_eq!(result, StepOutcome::Complete(Err(Error::Reset)));
    let channel = replacement.channels.get(1).ok_or("channel")?;
    assert_eq!(channel.resident, current.work);
    assert!(channel.work.is_some());
    assert_eq!(replacement.active, current.work);
    Ok(())
}
