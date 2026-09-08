use super::*;
use std::future::{Future, poll_fn};

#[tokio::test]
async fn register_observer_runs_after_effect_and_before_wake() -> support::TestResult {
    // Given: committed rings and a callback-gated observer on the activation command.
    let (handle, observer, memory) = support::gated_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let regions = handle.commit_memory_stage(stage).await?;
    support::program_activation(&handle, &regions).await?;
    let before = handle.snapshot().await;
    let mut wake = Box::pin(handle.wait_for_change_after(before));
    assert!(poll_once_pending(&mut wake).await);

    // When: activation pauses inside its observer before publishing the runner wake.
    let command_handle = handle.clone();
    let command = std::thread::spawn(move || -> support::TestResult {
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(
                command_handle
                    .register_endpoint()
                    .write(0x164, &1_u32.to_le_bytes()),
            )?;
        Ok(())
    });
    observer.await_observation();
    let regions_visible = memory.regions().len() == 3;
    let before_wake = poll_once_pending(&mut wake).await;
    observer.release_observation();
    command.join().map_err(|_| "command thread panicked")??;
    let published = wake.await;

    // Then: mappings precede observation, observation precedes wake, and activation commits.
    assert!(regions_visible && before_wake);
    assert_eq!(published.generation(), before.generation() + 1);
    let observations = observer.snapshot();
    let command = observations.last().ok_or("activation observation")?;
    assert_eq!(command.offset, 0x164);
    assert_eq!(command.request_bytes, 1_u32.to_le_bytes());
    assert_eq!(command.origin, AccessOrigin::Direct);
    assert_eq!(
        support::queue_status(&handle, activation.queue())
            .await?
            .queue(),
        activation.queue()
    );
    Ok(())
}

async fn poll_once_pending<F: Future>(future: &mut std::pin::Pin<Box<F>>) -> bool {
    poll_fn(|context| std::task::Poll::Ready(future.as_mut().poll(context).is_pending())).await
}
