use super::support::BlockingPort;
use pdma_model_contract::{BufferId, CommandId, CopyRequest, ModelError, ModelResult, QueueId};
use pdma_model_device::{AsyncDevice, AsyncStepStatus};
use pdma_model_host::{AsyncDriver, SoftwareCpu};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
};

#[derive(Clone, Copy)]
enum InvalidContext {
    Inactive,
    WrongCpu,
}

impl InvalidContext {
    const fn error(self) -> ModelError {
        match self {
            Self::Inactive => ModelError::SoftwareCpuInactive,
            Self::WrongCpu => ModelError::SoftwareCpuMismatch,
        }
    }

    async fn resume<F: Future>(self, future: Pin<&mut F>) -> F::Output {
        match self {
            Self::Inactive => future.await,
            Self::WrongCpu => SoftwareCpu::new().run_main(future).await,
        }
    }
}

async fn poll_pending_on<F: Future>(cpu: &SoftwareCpu, mut future: Pin<&mut F>) {
    cpu.run_main(poll_fn(|context| {
        assert!(future.as_mut().poll(context).is_pending());
        Poll::Ready(())
    }))
    .await;
}

async fn setup_copy(
    cpu: &SoftwareCpu,
    port: &mut BlockingPort,
) -> ModelResult<(AsyncDriver, CopyRequest)> {
    cpu.run_main(async {
        let driver = AsyncDriver::setup_on(port, 1, 1).await?;
        let src = driver.allocate_buffer_on(port, 4).await?;
        let dst = driver.allocate_buffer_on(port, 4).await?;
        Ok((driver, CopyRequest::new(src, dst, 4)))
    })
    .await
}

async fn create_repoll(context: InvalidContext) {
    // Given: CPU-bound setup first-polled to Pending by its owner.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    let control = port.clone();
    control.block_create();
    let owner = SoftwareCpu::new();
    let mut setup = Box::pin(AsyncDriver::setup_on(&mut port, 1, 1));
    poll_pending_on(&owner, setup.as_mut()).await;
    control.allow_create();

    // When: the pending setup is repolled without its matching CPU.
    let invalid = context.resume(setup.as_mut()).await;
    drop(setup);
    assert!(matches!(invalid, Err(error) if error == context.error()));
    assert_eq!(control.create_calls(), 0);
    let retried = owner.run_main(AsyncDriver::setup_on(&mut port, 1, 1)).await;

    // Then: the inner create never ran and the owner still receives queue zero.
    assert_eq!(control.create_calls(), 1);
    assert_eq!(retried.map(|driver| driver.queue_id()), Ok(QueueId::new(0)));
}

async fn submit_repoll(context: InvalidContext) -> ModelResult<()> {
    // Given: one reserved command blocked before device acceptance.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    let control = port.clone();
    let owner = SoftwareCpu::new();
    let (mut driver, request) = setup_copy(&owner, &mut port).await?;
    control.block_submit();
    let mut submit = Box::pin(driver.submit_copy_on(&mut port, request));
    poll_pending_on(&owner, submit.as_mut()).await;
    control.allow_submit();

    // When: the pending submit is repolled without its matching CPU.
    let invalid = context.resume(submit.as_mut()).await;
    drop(submit);
    assert_eq!(invalid, Err(context.error()));
    assert_eq!(control.submit_calls(), 0);
    let retried = owner
        .run_main(driver.submit_copy_on(&mut port, request))
        .await;

    // Then: acceptance did not commit and rollback restores credit for command one.
    assert_eq!(control.submit_calls(), 1);
    assert_eq!(retried, Ok(CommandId::new(1)));
    Ok(())
}

async fn doorbell_repoll(context: InvalidContext) -> ModelResult<()> {
    // Given: one command accepted before its doorbell blocks.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    let control = port.clone();
    let owner = SoftwareCpu::new();
    let (mut driver, request) = setup_copy(&owner, &mut port).await?;
    control.block_doorbell();
    let mut submit = Box::pin(driver.submit_copy_on(&mut port, request));
    poll_pending_on(&owner, submit.as_mut()).await;
    control.allow_doorbell();

    // When: the pending doorbell is repolled without its matching CPU.
    let invalid = context.resume(submit.as_mut()).await;
    drop(submit);
    assert_eq!(invalid, Err(context.error()));
    assert_eq!(control.doorbell_calls(), 0);
    let second = owner
        .run_main(driver.submit_copy_on(&mut port, request))
        .await;

    // Then: acceptance remains committed but the runnable doorbell does not.
    assert_eq!(control.submit_calls(), 1);
    assert_eq!(control.doorbell_calls(), 0);
    assert_eq!(second, Err(ModelError::DriverQueueFull));
    Ok(())
}

async fn completion_repoll(context: InvalidContext) -> ModelResult<()> {
    // Given: one completion blocked after its driver's entry validation.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut port = BlockingPort::new(device.handle());
    let control = port.clone();
    let owner = SoftwareCpu::new();
    let (mut driver, request) = setup_copy(&owner, &mut port).await?;
    owner
        .run_main(driver.submit_copy_on(&mut port, request))
        .await?;
    assert_eq!(
        device.wait_for_progress_after(0).await.last_status,
        AsyncStepStatus::Progressed
    );
    control.block_completion();
    let mut poll = Box::pin(driver.poll_completion_on(&mut port));
    poll_pending_on(&owner, poll.as_mut()).await;
    control.allow_completion();

    // When: the pending destructive poll is repolled without its matching CPU.
    let invalid = context.resume(poll.as_mut()).await;
    drop(poll);
    assert_eq!(invalid, Err(context.error()));
    assert_eq!(control.completion_calls(), 0);
    let retried = owner
        .run_main(async {
            let completion = driver.poll_completion_on(&mut port).await?;
            let next = driver.submit_copy_on(&mut port, request).await;
            Ok::<_, ModelError>((
                completion.map(pdma_model_contract::CompletionRecord::command_id),
                next,
            ))
        })
        .await;
    device.request_shutdown().await;
    assert!(worker.await.is_ok());

    // Then: no inner pop occurred; the owner sees the record and reclaimed credit.
    assert_eq!(control.completion_calls(), 1);
    assert_eq!(
        retried,
        Ok((Some(CommandId::new(0)), Ok(CommandId::new(1))))
    );
    Ok(())
}

async fn event_repoll(context: InvalidContext) -> ModelResult<()> {
    // Given: one fault event blocked after its driver's entry validation.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut port = BlockingPort::new(device.handle());
    let control = port.clone();
    let owner = SoftwareCpu::new();
    let (mut driver, request) = setup_copy(&owner, &mut port).await?;
    let bad = CopyRequest::new(BufferId::new(usize::MAX), request.dst(), request.len());
    owner
        .run_main(driver.submit_copy_on(&mut port, bad))
        .await?;
    let _error = device.wait_for_error_after(0).await;
    control.block_event();
    let mut poll = Box::pin(driver.poll_event_on(&mut port));
    poll_pending_on(&owner, poll.as_mut()).await;
    control.allow_event();

    // When: the pending destructive poll is repolled without its matching CPU.
    let invalid = context.resume(poll.as_mut()).await;
    drop(poll);
    assert_eq!(invalid, Err(context.error()));
    assert_eq!(control.event_calls(), 0);
    let retried = owner
        .run_main(async {
            let event = driver.poll_event_on(&mut port).await?;
            let next = driver.submit_copy_on(&mut port, request).await;
            Ok::<_, ModelError>((
                event.map(pdma_model_contract::EventRecord::command_id),
                next,
            ))
        })
        .await;
    device.request_shutdown().await;
    assert!(worker.await.is_ok());

    // Then: no inner pop occurred; the owner sees the event and reclaimed credit.
    assert_eq!(control.event_calls(), 1);
    assert_eq!(
        retried,
        Ok((Some(CommandId::new(0)), Ok(CommandId::new(1))))
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn pending_create_repoll_rejects_inactive_cpu_without_creating_queue() {
    create_repoll(InvalidContext::Inactive).await;
}

#[tokio::test(flavor = "current_thread")]
async fn pending_create_repoll_rejects_wrong_cpu_without_creating_queue() {
    create_repoll(InvalidContext::WrongCpu).await;
}

#[tokio::test(flavor = "current_thread")]
async fn pending_submit_repoll_rejects_inactive_cpu_without_accepting_command() {
    assert_eq!(submit_repoll(InvalidContext::Inactive).await, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn pending_submit_repoll_rejects_wrong_cpu_without_accepting_command() {
    assert_eq!(submit_repoll(InvalidContext::WrongCpu).await, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn pending_doorbell_repoll_rejects_inactive_cpu_without_runnable_side_effect() {
    assert_eq!(doorbell_repoll(InvalidContext::Inactive).await, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn pending_doorbell_repoll_rejects_wrong_cpu_without_runnable_side_effect() {
    assert_eq!(doorbell_repoll(InvalidContext::WrongCpu).await, Ok(()));
}

#[tokio::test(flavor = "current_thread")]
async fn pending_completion_repoll_rejects_inactive_cpu_without_consuming_record() -> ModelResult<()>
{
    completion_repoll(InvalidContext::Inactive).await
}

#[tokio::test(flavor = "current_thread")]
async fn pending_completion_repoll_rejects_wrong_cpu_without_consuming_record() -> ModelResult<()> {
    completion_repoll(InvalidContext::WrongCpu).await
}

#[tokio::test(flavor = "current_thread")]
async fn pending_event_repoll_rejects_inactive_cpu_without_consuming_record() -> ModelResult<()> {
    event_repoll(InvalidContext::Inactive).await
}

#[tokio::test(flavor = "current_thread")]
async fn pending_event_repoll_rejects_wrong_cpu_without_consuming_record() -> ModelResult<()> {
    event_repoll(InvalidContext::WrongCpu).await
}
