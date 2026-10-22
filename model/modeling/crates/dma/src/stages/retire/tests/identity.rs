use super::{TestResult, queued_channel};
use crate::{CheckRanges, CopyProgress, Error, Fetch, Flow, Retire};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use pdma_model_contract::dma::ChannelId;

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
    Err("identity test watchdog".into())
}

#[test]
fn substituted_progress_cannot_select_a_different_channel_than_its_flow_owner() -> TestResult {
    // Given: real validated progress and independently owned flow from different channels.
    let mut owner = queued_channel(ChannelId::new(0))?;
    let mut other = queued_channel(ChannelId::new(1))?;
    let request = owner.command_request(ChannelId::new(0))?;
    let fetched = finish(&mut Fetch, &mut owner, request)?;
    let original_flow = finish(
        &mut CheckRanges::default(),
        &mut owner.execution_context(),
        fetched,
    )?;
    let request = other.command_request(ChannelId::new(1))?;
    let fetched = finish(&mut Fetch, &mut other, request)?;
    let different = finish(
        &mut CheckRanges::default(),
        &mut other.execution_context(),
        fetched,
    )?;
    let mut saved: Option<CopyProgress> = None;
    let other_owner = different.map(|progress| saved = Some(progress)).finish();
    let progress = saved.ok_or("validated progress")?;
    let substituted: Flow<CopyProgress> = original_flow.map(|_| progress);
    // When: a custom transform substitutes the other channel's genuine progress value.
    let output = other.execution_context().install_progress(substituted);
    // Then: selection follows the flow owner and rejects the nested channel mismatch.
    assert_eq!(output.status(), Err(Error::Routing));
    assert!(
        other
            .channels
            .get(1)
            .ok_or("channel")?
            .work
            .as_ref()
            .ok_or("accepted")?
            .progress
            .is_none()
    );
    let mut failed = Some(vec![(0, output.finish())]);
    assert_eq!(
        Retire.step(&mut owner, &mut failed)?,
        StepOutcome::Complete(Err(Error::Routing))
    );
    finish(&mut Retire, &mut other, vec![(0, other_owner)])?;
    Ok(())
}
