#![allow(missing_docs, clippy::redundant_pub_crate)]

#[path = "mmio_queue/support.rs"]
mod support;

use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::abi::{WholeDeviceReset, decode_event};
use pdma_model_contract::{DeviceEpoch, QueueId};
use pdma_model_device::AsyncDevice;
use support::{
    RawActivation, activate_raw, issue_queue_command, program_activation, read_response, read_u32,
    read_u64,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

include!("mmio_queue/activation_rejections.rs");

#[tokio::test]
async fn raw_activation_uses_exact_committed_mappings_and_returns_zero_cursors() -> TestResult {
    // Given: exact committed depth-two SQ/CQ/EQ mappings and programmed raw registers.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let committed = handle.commit_dma_stage(stage).await?;

    // When: ACTIVATE is issued solely through the raw queue register window.
    activate_raw(&handle, DeviceEpoch::initial(), QueueId::new(0), &committed).await?;

    // Then: the complete response bank reports the queue and seven zero cursor shadows.
    assert_eq!(read_u32(&handle, 0x16c).await?, MmioResult::Ok.code());
    assert_eq!(read_u64(&handle, 0x188).await?, 0);
    assert_eq!(read_u64(&handle, 0x190).await?, 0);
    for offset in (0x198..=0x1c8).step_by(8) {
        assert_eq!(read_u64(&handle, offset).await?, 0);
    }
    Ok(())
}

#[tokio::test]
async fn raw_activation_rejects_wrong_mapping_id_and_stale_epoch_without_mutation() -> TestResult {
    // Given: exact mappings paired with misaligned, wrong-ID, and stale activation registers.
    let (wrong_device, _worker) = AsyncDevice::queue_port();
    let wrong = wrong_device.handle();
    let wrong_stage = wrong.stage_dma_regions(&[128, 64, 128]).await?;
    let wrong_regions = wrong.commit_dma_stage(wrong_stage).await?;
    let [wrong_submission, wrong_completion, wrong_event] = wrong_regions.as_slice() else {
        return Err("three committed queue regions required".into());
    };
    program_activation(
        &wrong,
        RawActivation::depth_two(
            DeviceEpoch::initial(),
            QueueId::new(0),
            [*wrong_submission, *wrong_completion, *wrong_event],
        ),
    )
    .await?;

    // When: a programmed SQ base violates the canonical submission alignment.
    support::write_u64(
        &wrong,
        0x138,
        wrong_submission.base().raw().saturating_add(1),
    )
    .await?;
    issue_queue_command(&wrong, 1).await?;
    assert_eq!(
        read_u32(&wrong, 0x16c).await?,
        MmioResult::MisalignedRing.code()
    );

    // Then: every rejection preserves the next ID and reset-persistent mappings for valid reuse.
    activate_raw(
        &wrong,
        DeviceEpoch::initial(),
        QueueId::new(1),
        &wrong_regions,
    )
    .await?;
    assert_eq!(
        read_u32(&wrong, 0x16c).await?,
        MmioResult::InvalidQueue.code()
    );
    activate_raw(
        &wrong,
        DeviceEpoch::initial(),
        QueueId::new(0),
        &wrong_regions,
    )
    .await?;
    assert_eq!(read_u32(&wrong, 0x16c).await?, MmioResult::Ok.code());

    let current = wrong
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    activate_raw(
        &wrong,
        DeviceEpoch::initial(),
        QueueId::new(0),
        &wrong_regions,
    )
    .await?;
    assert_eq!(
        read_u32(&wrong, 0x16c).await?,
        MmioResult::StaleEpoch.code()
    );
    support::write_u64(&wrong, 0x118, 1).await?;
    issue_queue_command(&wrong, 3).await?;
    assert_eq!(
        read_u32(&wrong, 0x16c).await?,
        MmioResult::StaleEpoch.code()
    );
    activate_raw(&wrong, current, QueueId::new(0), &wrong_regions).await?;
    assert_eq!(read_u32(&wrong, 0x16c).await?, MmioResult::Ok.code());
    Ok(())
}

#[tokio::test]
async fn raw_status_and_cursor_commands_update_all_seven_shadows_atomically() -> TestResult {
    // Given: one mapped active depth-two queue with every cursor at zero.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let regions = handle.commit_dma_stage(stage).await?;
    activate_raw(&handle, DeviceEpoch::initial(), QueueId::new(0), &regions).await?;
    support::write_u64(&handle, 0x118, 1).await?;

    // When: all seven cursor commands advance their selected shadow from zero to one.
    for command in 3..=9 {
        issue_queue_command(&handle, command).await?;
        assert_eq!(read_u32(&handle, 0x16c).await?, MmioResult::Ok.code());
    }
    issue_queue_command(&handle, 2).await?;

    // Then: one raw status snapshot contains the same epoch, ID, and seven cursor values.
    let snapshot = read_response(&handle).await?;
    assert_eq!(snapshot, [0, 0, 1, 1, 1, 1, 1, 1, 1]);
    println!("{{\"seven_cursor_status\":{snapshot:?}}}");

    let preserved = read_response(&handle).await?;
    support::write_u64(&handle, 0x110, 0).await?;
    support::write_u64(&handle, 0x118, 2).await?;
    issue_queue_command(&handle, 7).await?;
    assert_eq!(
        read_u32(&handle, 0x16c).await?,
        MmioResult::InvalidTransaction.code()
    );
    assert_eq!(read_response(&handle).await?, preserved);

    support::write_u64(&handle, 0x110, 1).await?;
    support::write_u64(&handle, 0x118, 0).await?;
    issue_queue_command(&handle, 7).await?;
    assert_eq!(
        read_u32(&handle, 0x16c).await?,
        MmioResult::CursorUpdateRegression.code()
    );
    assert_eq!(read_response(&handle).await?, preserved);

    support::write_u64(&handle, 0x110, 1).await?;
    support::write_u64(&handle, 0x118, 0).await?;
    issue_queue_command(&handle, 3).await?;
    assert_eq!(
        read_u32(&handle, 0x16c).await?,
        MmioResult::CursorUpdateRegression.code()
    );
    assert_eq!(read_u64(&handle, 0x180).await?, 0);
    assert_eq!(read_response(&handle).await?, preserved);

    support::write_u64(&handle, 0x110, 1).await?;
    support::write_u64(&handle, 0x118, u64::MAX).await?;
    issue_queue_command(&handle, 3).await?;
    assert_eq!(
        read_u32(&handle, 0x16c).await?,
        MmioResult::CursorDistanceExceeded.code()
    );
    assert_eq!(read_response(&handle).await?, preserved);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn raw_publication_precedes_doorbell_and_malformed_descriptor_retires_as_event() -> TestResult
{
    // Given: an active queue whose first committed descriptor slot contains malformed bytes.
    let (device, worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let regions = handle.commit_dma_stage(stage).await?;
    let [submission, _, event] = regions.as_slice() else {
        return Err("three committed queue regions required".into());
    };
    activate_raw(&handle, DeviceEpoch::initial(), QueueId::new(0), &regions).await?;
    handle.write_iova(submission.base(), &[0; 64]).await?;

    // When: doorbell-before-publication rejects, publication succeeds, then capture succeeds.
    support::write_u64(&handle, 0x118, 1).await?;
    issue_queue_command(&handle, 4).await?;
    assert_eq!(
        read_u32(&handle, 0x16c).await?,
        MmioResult::InvalidTransaction.code()
    );
    issue_queue_command(&handle, 2).await?;
    assert_eq!(read_response(&handle).await?[2..5], [0, 0, 0]);
    issue_queue_command(&handle, 3).await?;
    assert_eq!(read_response(&handle).await?[2..5], [1, 0, 0]);
    assert_eq!(handle.pending_dma_result().await, None);
    support::write_u64(&handle, 0x110, 0).await?;
    issue_queue_command(&handle, 4).await?;
    let worker = tokio::spawn(worker.run());
    handle.wait_for_progress_after(0).await;

    // Then: the existing retirement core consumes one SQ slot and publishes one fault event.
    issue_queue_command(&handle, 2).await?;
    let retired = read_response(&handle).await?;
    assert_eq!(retired[2..5], [1, 1, 1]);
    assert_eq!(retired[7], 1);
    println!("{{\"malformed_retirement\":{retired:?}}}");
    let bytes = handle.read_iova(event.base(), 64).await?;
    assert!(decode_event(&bytes).is_ok());
    handle.request_shutdown().await;
    worker.await?;
    Ok(())
}
