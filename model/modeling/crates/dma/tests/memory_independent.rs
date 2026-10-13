//! Memory-independent stages retain context, reset binding and IRQ composition.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{AfterEach, Operation, RunError, Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{ChannelContext, Dma, Error, Fetch, Resettable, UpdateIrq};
use support::{TestResult, command, setup, submit};

struct CommandId;

impl<M, S> Stage<ChannelContext<'_, M, S>> for CommandId {
    type Input = pdma_model_dma::CommandInput;
    type Output = u64;
    type Error = Error;
    type State = Option<pdma_model_dma::CommandInput>;

    fn start(command: Self::Input) -> Self::State {
        Some(command)
    }

    fn step(
        &mut self,
        _: &mut ChannelContext<'_, M, S>,
        state: &mut Self::State,
    ) -> StepResult<u64, Error> {
        state.take().map_or(Err(RunError::Exhausted), |command| {
            Ok(StepOutcome::Complete(Ok(command.command().id)))
        })
    }
}

#[test]
fn reset_and_irq_composition_requires_no_memory_backend() -> TestResult {
    // Given: a real fetched command and a downstream context with no memory capability.
    let (mut source, memory) = setup(0x1100, 2)?;
    let work = command();
    let expected = work.id;
    submit(&mut source, &memory, work)?;
    let channel = ChannelId::new(0);
    let task = match Operation::new(channel).step(&mut Fetch, &mut source)? {
        StepOutcome::Complete(result) => result?,
        outcome @ StepOutcome::InProgress => {
            return Err(format!("command not accepted: {outcome:?}").into());
        }
    };
    let mut device = Dma::with_sideband((), 1, std::convert::identity)?;
    let mut stage = Resettable::new(AfterEach::new(CommandId, UpdateIrq));
    // When: the bound composition transforms that command without reading memory.
    let outcome = Operation::new(task).step(&mut stage, &mut device)?;
    // Then: context access and IRQ refresh do not impose an unused memory trait.
    let StepOutcome::Complete(result) = outcome else {
        return Err("memory-independent stage did not complete".into());
    };
    let output = result?;
    assert_eq!(output.channel(), channel);
    assert_eq!(output.value, expected);
    assert!(!device.channel_context(channel)?.halted());
    Ok(())
}
