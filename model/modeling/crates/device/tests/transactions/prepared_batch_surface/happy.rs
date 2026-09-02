use super::*;
use std::future::{Future, poll_fn};

#[tokio::test]
async fn prepared_batch_surface() -> support::TestResult {
    // Given: both closed batches share a callback-gated observer and one memory store.
    let (handle, observer, memory) = support::gated_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    let submission = *submission;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let mut bootstrap_wake = Box::pin(handle.wait_for_change_after(handle.snapshot().await));
    assert!(poll_once_pending(&mut bootstrap_wake).await);

    // When: each batch blocks inside its observer after installs and before wake publication.
    let bootstrap_handle = handle.clone();
    let bootstrap = std::thread::spawn(move || -> support::TestResult<_> {
        Ok(tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(
                bootstrap_handle.bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation)),
            )?)
    });
    observer.await_observation();
    let bootstrap_installed = memory.regions().len() == 3;
    let bootstrap_before_wake = poll_once_pending(&mut bootstrap_wake).await;
    assert!(bootstrap_installed && bootstrap_before_wake);
    observer.release_observation();
    let bootstrap_response = bootstrap
        .join()
        .map_err(|_| "bootstrap thread panicked")??;
    let _ = bootstrap_wake.await;

    let submission_before = handle.snapshot().await;
    let mut submission_wake = Box::pin(handle.wait_for_change_after(submission_before));
    assert!(poll_once_pending(&mut submission_wake).await);
    let submission_handle = handle.clone();
    let request = support::submission(
        activation.queue(),
        RingCursor::new(0),
        submission.base(),
        0x6b,
    );
    let submission_thread = std::thread::spawn(move || -> support::TestResult<_> {
        Ok(tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(submission_handle.submit_queue_batch(request))?)
    });
    observer.await_observation();
    let submission_installed = memory.regions().len() == 3;
    let submission_before_wake = poll_once_pending(&mut submission_wake).await;
    assert!(submission_installed && submission_before_wake);
    observer.release_observation();
    let submission_response = submission_thread
        .join()
        .map_err(|_| "submission thread panicked")??;
    let _ = submission_wake.await;

    // Then: the literal commands, responses, bytes, origins, and sequences are exact.
    let observations: [_; 2] = observer
        .snapshot()
        .try_into()
        .map_err(|_| "exactly two observations required")?;
    let expected = support::response(support::queue_status(&handle, activation.queue()).await?)
        .to_bytes()
        .map(Into::into)
        .map_err(|_| "submission response encode failed")?;
    let responses_exact =
        submission_response == expected && bootstrap_response.as_bytes().len() == 72;
    let commands = observations
        .each_ref()
        .map(|item| {
            item.request_bytes
                .as_slice()
                .try_into()
                .map(u32::from_le_bytes)
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let [bootstrap_command, submission_command] = commands.as_slice() else {
        return Err("two command values required".into());
    };
    assert_eq!([*bootstrap_command, *submission_command], [1, 3]);
    assert!(observations.iter().all(|item| {
        item.origin == AccessOrigin::Batch && item.offset == 0x168 && item.width == 4
    }));
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0x6b; 64]
    );
    assert!(responses_exact);
    let installs_before_observe = bootstrap_installed && submission_installed;
    let observe_before_wake = bootstrap_before_wake
        && submission_before_wake
        && handle.snapshot().await.generation() == submission_before.generation() + 1;
    assert!(installs_before_observe && observe_before_wake);
    println!(
        "bootstrap_command={bootstrap_command};submission_command={submission_command};bootstrap_observations=1;submission_observations=1;offsets={:#x}/{},{:#x}/{};origins=batch,batch;responses_exact={responses_exact};installs_before_observe={installs_before_observe};observe_before_wake={observe_before_wake};sequences={},{}",
        observations[0].offset,
        observations[0].width,
        observations[1].offset,
        observations[1].width,
        observations[0].sequence,
        observations[1].sequence,
    );
    Ok(())
}

async fn poll_once_pending<F: Future>(future: &mut std::pin::Pin<Box<F>>) -> bool {
    poll_fn(|context| std::task::Poll::Ready(future.as_mut().poll(context).is_pending())).await
}
