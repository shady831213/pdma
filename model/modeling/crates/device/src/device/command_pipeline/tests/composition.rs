use std::num::NonZeroUsize;

use hardware_interface_pipeline::{
    ImmediateExecutor, Operation, PipelineExt, RunOutcome, Stage, StepOutcome,
};
use pdma_model_contract::semantic::{CommandId, CompletionRecord, CompletionStatus};

use super::support::{Case, prepare, read};
use crate::{AsyncStallReason, HardwareStepStatus, ModelError};

#[derive(Default)]
struct Observer {
    statuses: Vec<HardwareStepStatus>,
}

impl Stage<()> for Observer {
    type Input = HardwareStepStatus;
    type Output = HardwareStepStatus;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = Option<HardwareStepStatus>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        (): &mut (),
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(status) = state.take() else {
            return StepOutcome::Exhausted;
        };
        self.statuses.push(status);
        StepOutcome::Complete(Ok(status))
    }
}

#[test]
fn device_composes_with_downstream_observer_and_reuses_hardware() {
    // Given: real queued DMA work and an observer connected to the device's output.
    let prepared = prepare(Case::Copy);
    let mut hardware = prepared.fixture.core.then(Observer::default());
    let mut operation = Operation::new(());
    let executor = ImmediateExecutor::new(NonZeroUsize::new(16).expect("nonzero budget"));

    // When: the composite handles the copy and then a fresh idle operation.
    assert_eq!(
        executor.run(&mut hardware, &mut operation, &mut ()),
        Ok(RunOutcome::Complete(Ok(HardwareStepStatus::Progressed)))
    );
    assert_eq!(
        hardware.downstream.statuses,
        [HardwareStepStatus::Progressed]
    );
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        StepOutcome::Exhausted
    );
    let mut next = Operation::new(());
    assert_eq!(
        executor.run(&mut hardware, &mut next, &mut ()),
        Ok(RunOutcome::Complete(Ok(HardwareStepStatus::Idle)))
    );

    // Then: one DMA and completion are visible, and both outputs reached the same observer.
    assert_eq!(
        read(&hardware.upstream, prepared.destinations[0]),
        Ok(vec![1, 2, 3, 4, 0, 0, 0, 0])
    );
    assert_eq!(
        hardware.upstream.poll_completion(prepared.queue),
        Ok(Some(CompletionRecord::new(
            prepared.queue.queue_id(),
            CommandId::new(7),
            CompletionStatus::Success,
        )))
    );
    assert_eq!(hardware.upstream.poll_completion(prepared.queue), Ok(None));
    assert_eq!(hardware.upstream.native_steps(), 1);
    assert_eq!(
        hardware.downstream.statuses,
        [HardwareStepStatus::Progressed, HardwareStepStatus::Idle]
    );
    println!(
        "DEVICE_COMPOSITION|dma=[1,2,3,4,0,0,0,0]|completions=1|fetches=1|observer=[Progressed,Idle]|completed_operation=Exhausted"
    );
}
