use super::*;
use std::future::{Future, poll_fn};

#[tokio::test]
async fn register_observer_runs_after_effect_and_before_wake() -> support::TestResult {
    // Given: committed rings and a callback-gated observer on queue enable.
    let (handle, observer, memory) = support::gated_handle();
    let regions = handle.rings([128, 64, 128])?;
    support::program_activation(&handle.register_endpoint(), &regions).await?;
    let before = handle.snapshot().await;
    let mut wake = Box::pin(handle.wait_for_change_after(before));
    assert!(poll_once_pending(&mut wake).await);

    // When: activation pauses inside its observer before publishing the runner wake.
    let command_handle = handle.clone();
    let command = std::thread::spawn(move || -> support::TestResult {
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(command_handle.register_endpoint().write(
                queue_offset(0, QueueRegisters::ENABLE.offset())?,
                &1_u32.to_le_bytes(),
            ))?;
        Ok(())
    });
    observer.await_observation();
    let regions_visible = memory.regions().len() == 3;
    let committed = handle.snapshot().await;
    let before_wake = poll_once_pending(&mut wake).await;
    observer.release_observation();
    command.join().map_err(|_| "command thread panicked")??;
    let published = wake.await;

    // Then: mappings precede observation, observation precedes wake, and activation commits.
    assert!(regions_visible && before_wake);
    assert_eq!(committed.generation(), before.generation() + 1);
    assert_eq!(published.generation(), before.generation() + 1);
    let observations = observer.snapshot();
    let command = observations.last().ok_or("activation observation")?;
    assert_eq!(
        command.offset,
        queue_offset(0, QueueRegisters::ENABLE.offset())?
    );
    assert_eq!(command.request_bytes, 1_u32.to_le_bytes());
    assert_eq!(command.origin, AccessOrigin::Direct);
    let mut enabled = [0; 4];
    handle
        .register_endpoint()
        .read(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &mut enabled,
        )
        .await?;
    assert_eq!(enabled, 1_u32.to_le_bytes());
    Ok(())
}

async fn poll_once_pending<F: Future>(future: &mut std::pin::Pin<Box<F>>) -> bool {
    poll_fn(|context| std::task::Poll::Ready(future.as_mut().poll(context).is_pending())).await
}
