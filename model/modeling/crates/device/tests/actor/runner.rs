use super::support::Fixture;
use pdma_model_contract::{
    abi::{AbiCompletion, registers::QueueRegisters},
    semantic::ResourceConfig,
};

mod ranges;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_retires_copy_through_selected_hardware_queue() {
    // Given: a mapped hardware queue and one exact copy descriptor.
    let (fixture, worker) = Fixture::new(ResourceConfig::default()).await;
    fixture
        .handle
        .write_memory(fixture.source, &[1, 2, 3, 4])
        .await
        .expect("source");
    fixture.copy(0, fixture.payload(4)).await;
    let worker = tokio::spawn(worker.run());

    // When: software publishes and doorbells the descriptor.
    fixture.publish(1).await;
    let snapshot = fixture.device.wait_for_progress_after(0).await;
    fixture.device.request_shutdown().await;
    let summary = worker.await.expect("worker joins");

    // Then: the selected CQ records success and the worker counts one retirement.
    assert_eq!(fixture.completion(0).await, AbiCompletion::success(0, 0));
    assert_eq!(
        fixture.cursor(QueueRegisters::CQ_PRODUCER.offset()).await,
        1
    );
    assert_eq!(
        fixture
            .handle
            .read_memory(fixture.destination, 4)
            .await
            .expect("destination"),
        [1, 2, 3, 4]
    );
    assert_eq!((snapshot.steps, summary.steps, summary.errors), (1, 1, 0));
}

#[tokio::test(flavor = "current_thread")]
async fn pending_completion_retry_after_doorbell_has_no_fake_progress() {
    // Given: three copies sharing one completion publication credit.
    let (fixture, worker) =
        Fixture::new(ResourceConfig::default().with_completion_publication_slots(1)).await;
    fixture
        .handle
        .write_memory(fixture.source, &[3, 5, 7, 9])
        .await
        .expect("source");
    for slot in 0..3 {
        fixture.copy(slot, fixture.payload(4)).await;
    }
    let worker = tokio::spawn(worker.run());

    // When: software frees CQ credits while retirement retries.
    fixture.publish(3).await;
    let first = fixture.device.wait_for_progress_after(0).await;
    tokio::task::yield_now().await;
    let stalled = fixture.device.snapshot().await;
    fixture
        .write_queue(QueueRegisters::DOORBELL.offset(), &3_u64.to_le_bytes())
        .await;
    for consumed in 1_u64..=2 {
        fixture
            .write_queue(
                QueueRegisters::CQ_CONSUMER.offset(),
                &consumed.to_le_bytes(),
            )
            .await;
        fixture
            .device
            .wait_for_progress_after(usize::try_from(consumed).expect("step"))
            .await;
    }
    fixture.device.request_shutdown().await;
    let summary = worker.await.expect("worker joins");

    // Then: only actual retirements count as progress and every descriptor completes once.
    assert_eq!(first.steps, 1);
    assert!(stalled.stalls >= 1);
    assert_eq!((summary.steps, summary.errors), (3, 0));
    for slot in 0..3 {
        assert_eq!(
            fixture.completion(slot).await,
            AbiCompletion::success(0, slot)
        );
    }
    assert_eq!(
        fixture
            .handle
            .read_memory(fixture.destination, 4)
            .await
            .expect("destination"),
        [3, 5, 7, 9]
    );
}
