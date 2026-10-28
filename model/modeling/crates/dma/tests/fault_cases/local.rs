use super::*;
use hw_model_pipeline::Stage;
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
    let fetched = finish(&mut Fetch, &mut dma, Some(CommandRequest::Fetch(snapshot)))?;
    let mut context = dma.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, fetched)?;
    let request = context.install_progress(progress);
    let chunk = finish(&mut PrepareChunk::default(), &mut context, request)?;
    dma.memory
        .missing
        .set(Some((if write { 0x1300 } else { 0x1200 }, write)));
    let mut context = dma.execution_context();
    let mut payload = finish(&mut ReadCopy::default(), &mut context, chunk)?;
    let parked = if write {
        payload
            .data_mut()
            .ok_or("payload")?
            .bytes_mut()?
            .copy_from_slice(b"WXYZ");
        let completion = finish(&mut WriteCopy::default(), &mut context, payload)?;
        assert!(completion.data().is_none());
        completion.finish()
    } else {
        assert!(payload.data().is_none());
        payload.finish()
    };
    finish(&mut pdma_model_dma::Retire, &mut dma, vec![(0, parked)])?;
    assert_eq!(fault_status(&mut dma)?, 1);
    dma.memory.missing.set(None);
    resume(&mut dma, 1)?;
    let accesses = dma.memory.accesses.get();
    let empty = finish(&mut Fetch, &mut dma, None)?;
    let mut context = dma.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, empty)?;
    let request = context.install_progress(progress);
    let empty = finish(&mut PrepareChunk::default(), &mut context, request)?;
    let empty = finish(&mut ReadCopy::default(), &mut context, empty)?;
    let empty = finish(&mut WriteCopy::default(), &mut context, empty)?;
    assert!(empty.data().is_none());
    assert_eq!(
        dma.memory.accesses.get(),
        accesses,
        "resume alone does not grant execution"
    );
    // When: a new admission grants the parked owner, then its local leaf gets a bubble.
    let request = dma.command_request(ChannelId::new(0))?;
    let resumed = finish(&mut Fetch, &mut dma, request)?;
    let mut context = dma.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, resumed)?;
    let request = context.install_progress(progress);
    let chunk = finish(&mut PrepareChunk::default(), &mut context, request)?;
    assert_eq!(
        dma.memory.accesses.get(),
        accesses,
        "regrant must not execute payload"
    );
    let mut context = dma.execution_context();
    let payload = finish(&mut ReadCopy::default(), &mut context, chunk)?;
    if !write {
        assert_eq!(payload.data().ok_or("read checkpoint")?.bytes()?, b"abcd");
    }
    let completion = finish(&mut WriteCopy::default(), &mut context, payload)?;
    // Then: the leaf completes the original command with exact retained bytes.
    assert_eq!(
        completion.data().ok_or("completion")?.channel(),
        ChannelId::new(0)
    );
    crate::support::assert_no_event(&mut dma)?;
    assert_eq!(
        dma.memory
            .memory
            .read_buffer::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
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
