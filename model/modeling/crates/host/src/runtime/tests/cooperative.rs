use super::{IRQ_ENABLE, Model, TestResult};
use hardware_interface_pipeline::{AsyncExecutor, Operation, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_dma::{Dma, Error};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Waker},
};

struct ObserveEnable(Arc<AtomicUsize>);

impl Stage<Dma<()>> for ObserveEnable {
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, device: &mut Dma<()>, (): &mut ()) -> StepResult<(), Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let mut value = [0; 4];
        Ok(match device.read(IRQ_ENABLE, &mut value) {
            Ok(()) if u32::from_le_bytes(value) != 0 => StepOutcome::Complete(Ok(())),
            Ok(()) => StepOutcome::InProgress,
            Err(error) => StepOutcome::Complete(Err(error)),
        })
    }
}

#[tokio::test]
async fn driver_yields_after_a_committed_step_and_software_can_change_hardware() -> TestResult {
    // Given: hardware waiting for a register change, with no wait notification protocol.
    let model = Model::new(Dma::new((), 1)?);
    let executor = AsyncExecutor::new(&model);
    let visits = Arc::new(AtomicUsize::new(0));
    let mut hardware = ObserveEnable(Arc::clone(&visits));
    let mut operation = Operation::new(());
    let before = visits.load(Ordering::Relaxed);
    // When: one driver poll returns control and software performs the enabling write.
    {
        let mut running = Box::pin(executor.run(&mut hardware, &mut operation));
        assert!(
            running
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    assert_eq!(visits.load(Ordering::Relaxed), before + 1);
    model
        .write(u32::try_from(IRQ_ENABLE)?, &1_u32.to_le_bytes())
        .await?;
    // Then: the next driving session observes the write and finishes the same operation.
    assert_eq!(executor.run(&mut hardware, &mut operation).await, Ok(()));
    Ok(())
}
