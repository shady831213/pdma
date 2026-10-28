//! Actual stage values compose over selected hardware resources.
/// Shared raw-bus fixtures.
pub mod support;

use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use hw_model_pipeline::{Operation, Stage, StepOutcome};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{CheckRanges, CommandRequest, Fetch, PrepareChunk, ReadCopy, WriteCopy};
use support::{TestResult, command, setup, submit};

#[test]
fn stage_outputs_carry_descriptor_addresses_bytes_and_completion() -> TestResult {
    // Given: a real command and source bytes in shared memory.
    let (mut dma, memory) = setup(0x1100, 2)?;
    submit(&mut dma, &memory, command())?;
    memory.write(0x1200.into(), MemoryAlignment::ONE, b"abcd")?;
    let Some(CommandRequest::Fetch(snapshot)) = dma.command_request(ChannelId::new(0))? else {
        return Err("missing fresh snapshot".into());
    };
    // When: standalone leaves transfer owned values through a narrow context.
    let fetched = finish(&mut Fetch, &mut dma, Some(CommandRequest::Fetch(snapshot)))?;
    assert_eq!(fetched.data().ok_or("fetch")?.command(), &command());
    let mut context = dma.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, fetched)?;
    let request = context.install_progress(progress);
    let chunk = finish(&mut PrepareChunk::default(), &mut context, request)?;
    let geometry = chunk.data().ok_or("chunk")?;
    assert_eq!(
        (
            geometry.source().raw(),
            geometry.destination().raw(),
            geometry.len()
        ),
        (0x1200, 0x1300, 4)
    );
    let mut payload = finish(&mut ReadCopy::default(), &mut context, chunk)?;
    assert_eq!(payload.data().ok_or("read")?.bytes()?, b"abcd");
    payload
        .data_mut()
        .ok_or("read")?
        .bytes_mut()?
        .copy_from_slice(b"WXYZ");
    let completion = finish(&mut WriteCopy::default(), &mut context, payload)?;
    // Then: downstream consumes the modified bytes and retains their owner identity.
    assert_eq!(
        completion.data().ok_or("completion")?.channel(),
        ChannelId::new(0)
    );
    support::assert_no_event(&mut dma)?;
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        *b"WXYZ"
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
    Err("stage exceeded bounded watchdog".into())
}
