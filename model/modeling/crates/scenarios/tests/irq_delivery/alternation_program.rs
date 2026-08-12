use super::support::{DriverBootstrap, HandlerIdentity, Milestone};
use pdma_model_contract::{CopyRequest, IrqRoute, IrqVector};
use pdma_model_device::AsyncDeviceHandle;
use pdma_model_host::ModelResult;
use pdma_model_host::{AsyncDriver, CpuLocal, SoftwareCpu};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[derive(Debug)]
pub struct MainOutcome {
    pub retained_submit: bool,
    pub final_depth: usize,
    pub irq_cleared: bool,
    pub copied: Vec<u8>,
}

#[derive(Debug)]
pub struct MainProgram {
    pub port: AsyncDeviceHandle,
    pub cpu: SoftwareCpu,
    pub depth: CpuLocal<usize>,
    pub bootstrap: oneshot::Sender<DriverBootstrap>,
    pub ready: oneshot::Receiver<()>,
    pub milestones: mpsc::Sender<Milestone>,
    pub a_done: mpsc::Receiver<()>,
    pub b_done: mpsc::Receiver<()>,
    pub vector_a: IrqVector,
    pub vector_b: IrqVector,
}

/// Spawns the complete modeled main program used by the alternating-vector story.
///
/// # Panics
/// Panics if a bounded bootstrap, readiness, milestone, or completion channel closes early.
pub fn spawn_main(mut program: MainProgram) -> JoinHandle<ModelResult<MainOutcome>> {
    tokio::spawn(async move {
        program
            .cpu
            .run_main(async move {
                let mut driver = AsyncDriver::setup_on(&mut program.port, 1, 4).await?;
                let completion_capability = driver.completion_irq_source();
                let event_capability = driver.event_irq_source();
                let completion_source = completion_capability.source();
                let event_source = event_capability.source();
                assert!(
                    program
                        .bootstrap
                        .send(DriverBootstrap {
                            driver: driver.clone(),
                            completion_source,
                            event_source,
                        })
                        .is_ok(),
                    "delivery bootstrap receives the retained capability clone"
                );
                assert!(program.ready.await.is_ok(), "every vector is registered");
                for (source, vector) in [
                    (completion_capability, program.vector_a),
                    (event_capability, program.vector_b),
                ] {
                    program
                        .port
                        .configure_irq_route(source, IrqRoute::Message(vector))
                        .await?;
                    program.port.enable_irq_source(source).await?;
                    program.port.unmask_irq_source(source).await?;
                }
                let src = driver.allocate_buffer_on(&mut program.port, 4).await?;
                let dst = driver.allocate_buffer_on(&mut program.port, 4).await?;
                driver
                    .write_buffer_on(&mut program.port, src, &[9, 8, 7, 6])
                    .await?;
                for round in 1..=2 {
                    assert!(
                        program
                            .milestones
                            .send(Milestone::MainSubmitA(round))
                            .await
                            .is_ok(),
                        "milestone channel stays open"
                    );
                    driver
                        .submit_copy_on(&mut program.port, CopyRequest::new(src, dst, 4))
                        .await?;
                    assert!(program.a_done.recv().await.is_some(), "handler A completes");
                    assert!(
                        program
                            .milestones
                            .send(Milestone::MainSubmitB(round))
                            .await
                            .is_ok(),
                        "milestone channel stays open"
                    );
                    driver
                        .submit_copy_on(&mut program.port, CopyRequest::new(src, dst, 8))
                        .await?;
                    assert!(program.b_done.recv().await.is_some(), "handler B completes");
                    assert!(
                        program
                            .milestones
                            .send(Milestone::MainResumed(round))
                            .await
                            .is_ok(),
                        "milestone channel stays open"
                    );
                }
                assert!(
                    program
                        .milestones
                        .send(Milestone::MainRetainedSubmit)
                        .await
                        .is_ok(),
                    "milestone channel stays open"
                );
                let retained_submit = driver
                    .submit_copy_on(&mut program.port, CopyRequest::new(src, dst, 4))
                    .await
                    .is_ok();
                assert!(
                    program.a_done.recv().await.is_some(),
                    "retained submit is handled"
                );
                Ok(MainOutcome {
                    retained_submit,
                    final_depth: program.depth.with(|depth| *depth)?,
                    irq_cleared: program.port.irq_snapshot().await.is_none(),
                    copied: program.port.read_buffer(dst, 0, 4).await?,
                })
            })
            .await
    })
}

/// Verifies and releases the deterministic handler/main milestone sequence.
///
/// # Panics
/// Panics if the observed sequence differs or the handler-release channel closes early.
pub async fn assert_milestone_order(
    milestones: &mut mpsc::Receiver<Milestone>,
    release_a: &mpsc::Sender<()>,
) {
    for round in 1..=2 {
        assert_eq!(milestones.recv().await, Some(Milestone::MainSubmitA(round)));
        assert_eq!(
            milestones.recv().await,
            Some(Milestone::HandlerStarted(HandlerIdentity::A, round))
        );
        assert!(milestones.try_recv().is_err());
        assert!(release_a.send(()).await.is_ok(), "handler A is released");
        assert_eq!(
            milestones.recv().await,
            Some(Milestone::HandlerCompleted(HandlerIdentity::A, round))
        );
        assert_eq!(milestones.recv().await, Some(Milestone::MainSubmitB(round)));
        assert_eq!(
            milestones.recv().await,
            Some(Milestone::HandlerStarted(HandlerIdentity::B, round))
        );
        assert_eq!(
            milestones.recv().await,
            Some(Milestone::HandlerCompleted(HandlerIdentity::B, round))
        );
        assert_eq!(milestones.recv().await, Some(Milestone::MainResumed(round)));
    }
    assert_eq!(milestones.recv().await, Some(Milestone::MainRetainedSubmit));
    assert_eq!(
        milestones.recv().await,
        Some(Milestone::HandlerStarted(HandlerIdentity::A, 3))
    );
    assert!(milestones.try_recv().is_err());
    assert!(
        release_a.send(()).await.is_ok(),
        "final handler A is released"
    );
    assert_eq!(
        milestones.recv().await,
        Some(Milestone::HandlerCompleted(HandlerIdentity::A, 3))
    );
}
