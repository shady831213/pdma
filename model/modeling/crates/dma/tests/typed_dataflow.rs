//! Actual stage values compose over selected hardware resources.
/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{CheckRanges, CommandRequest, Fetch, PrepareChunk, ReadCopy, WriteCopy};
use support::{TestResult, command, setup, submit};

#[test]
fn stage_outputs_carry_descriptor_addresses_bytes_and_response() -> TestResult {
    // Given: a real command and source bytes in shared memory.
    let (mut dma, memory) = setup(0x1100, 2)?;
    submit(&mut dma, &memory, command())?;
    memory.write(0x1200.into(), MemoryAlignment::ONE, b"abcd")?;
    let Some(CommandRequest::Fetch(snapshot)) = dma.command_request(ChannelId::new(0))? else {
        return Err("missing fresh snapshot".into());
    };
    // When: standalone leaves transfer owned values through a narrow context.
    let mut context = dma.execution_context(ChannelId::new(0))?;
    let fetched = finish(&mut Fetch, &mut context, Some(snapshot))?.ok_or("fetch")?;
    assert_eq!(fetched.command(), &command());
    let progress = finish(&mut CheckRanges, &mut context, Some(fetched))?.ok_or("validate")?;
    let request = context.install_progress(progress)?;
    let chunk = finish(&mut PrepareChunk, &mut context, Some(request))?.ok_or("chunk")?;
    assert_eq!(
        (chunk.source().raw(), chunk.destination().raw(), chunk.len()),
        (0x1200, 0x1300, 4)
    );
    let mut payload = finish(&mut ReadCopy, &mut context, Some(chunk))?.ok_or("read")?;
    assert_eq!(payload.bytes()?, b"abcd");
    payload.bytes_mut()?.copy_from_slice(b"WXYZ");
    let completion = finish(&mut WriteCopy, &mut context, Some(payload))?.ok_or("completion")?;
    // Then: downstream consumes the modified bytes and the actual response value.
    assert_eq!(completion.response().id, command().id);
    assert_eq!(completion.response().bytes_completed(), 4);
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
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
