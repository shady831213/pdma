use super::*;
use hardware_interface_pipeline::Stage;
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{CheckRanges, CommandRequest, Fetch, PrepareChunk, ReadCopy, WriteCopy};

#[test]
fn standalone_write_consumes_its_retained_transformed_checkpoint_after_regrant() -> TestResult {
    checkpoint(true)
}

#[test]
fn standalone_read_consumes_its_retained_chunk_after_regrant() -> TestResult {
    checkpoint(false)
}

fn checkpoint(write: bool) -> TestResult {
    // Given: standalone leaves park actual data at their own read or write boundary.
    let mut dma = setup()?.dma;
    dma.memory
        .memory
        .write(0x1200.into(), MemoryAlignment::ONE, b"abcd")?;
    submit(&mut dma, crate::support::command(), 0)?;
    let Some(CommandRequest::Fetch(snapshot)) = dma.command_request(ChannelId::new(0))? else {
        return Err("fresh descriptor missing".into());
    };
    let mut context = dma.execution_context(ChannelId::new(0))?;
    let fetched = finish(&mut Fetch, &mut context, Some(snapshot))?;
    let progress = finish(&mut CheckRanges, &mut context, fetched)?.ok_or("progress")?;
    let request = context.install_progress(progress)?;
    let chunk = finish(&mut PrepareChunk, &mut context, Some(request))?;
    dma.memory
        .missing
        .set(Some((if write { 0x1300 } else { 0x1200 }, write)));
    let mut context = dma.execution_context(ChannelId::new(0))?;
    let payload = finish(&mut ReadCopy, &mut context, chunk)?;
    if write {
        let mut payload = payload.ok_or("payload")?;
        payload.bytes_mut()?.copy_from_slice(b"WXYZ");
        assert!(finish(&mut WriteCopy, &mut context, Some(payload))?.is_none());
    } else {
        assert!(payload.is_none());
    }
    assert_eq!(fault_status(&mut dma)?, 1);
    dma.memory.missing.set(None);
    resume(&mut dma, 1)?;
    let accesses = dma.memory.accesses.get();
    let mut context = dma.execution_context(ChannelId::new(0))?;
    assert!(finish(&mut ReadCopy, &mut context, None)?.is_none());
    assert!(finish(&mut WriteCopy, &mut context, None)?.is_none());
    assert_eq!(
        dma.memory.accesses.get(),
        accesses,
        "resume alone does not grant execution"
    );
    // When: a new admission grants the parked owner, then its local leaf gets a bubble.
    let request = dma.command_request(ChannelId::new(0))?;
    let granted = finish(&mut pdma_model_dma::Grant, &mut dma, vec![(0, request)])?;
    assert!(matches!(granted, Some((_, None))));
    assert_eq!(
        dma.memory.accesses.get(),
        accesses,
        "regrant must not execute payload"
    );
    let mut context = dma.execution_context(ChannelId::new(0))?;
    let completion = if write {
        finish(&mut WriteCopy, &mut context, None)?
    } else {
        let payload = finish(&mut ReadCopy, &mut context, None)?.ok_or("read checkpoint")?;
        assert_eq!(payload.bytes()?, b"abcd");
        finish(&mut WriteCopy, &mut context, Some(payload))?
    }
    .ok_or("local checkpoint completion")?;
    // Then: the leaf completes the original command with exact retained bytes.
    assert_eq!(completion.response().bytes_completed(), 4);
    assert_eq!(
        dma.memory
            .memory
            .read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        if write { *b"WXYZ" } else { *b"abcd" }
    );
    assert_eq!(
        dma.memory
            .reads
            .borrow()
            .iter()
            .filter(|address| **address == 0x1200)
            .count(),
        if write { 1 } else { 2 }
    );
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
    Err("standalone stage watchdog".into())
}
