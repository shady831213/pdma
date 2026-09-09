use super::common::TestResult;
use crate::queue::bus_retirement::support::{Fixture, allocate_pair};
use pdma_model_contract::abi::AbiCompletion;
use pdma_model_contract::abi::registers::QueueRegisters;
use pdma_model_contract::semantic::ResourceConfig;

pub(super) async fn capture_and_nonzero_slot() -> TestResult {
    // Given: descriptor memory and producer publication alone do not capture work.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    let pair = allocate_pair(&fixture).await?;
    fixture.copy(0, pair).await?;
    assert_eq!(fixture.read(QueueRegisters::SQ_CAPTURED.offset()).await?, 0);
    assert_eq!(fixture.handle.native_steps().await, 0);
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 1).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_progress_after(0).await;
    fixture
        .cursor(QueueRegisters::CQ_CONSUMER.offset(), 1)
        .await?;
    fixture.handle.write_memory(pair[0], &[4, 3, 2, 1]).await?;

    // When: a second descriptor occupies a nonzero SQ slot and is captured.
    fixture.copy(1, pair).await?;
    assert_eq!(fixture.read(QueueRegisters::SQ_CAPTURED.offset()).await?, 1);
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 2).await?;
    fixture.handle.wait_for_progress_after(1).await;

    // Then: the second slot executes and its exact completion wraps the result ring.
    assert_eq!(fixture.read(QueueRegisters::SQ_CONSUMER.offset()).await?, 2);
    assert_eq!(fixture.handle.read_memory(pair[1], 4).await?, [4, 3, 2, 1]);
    let record = AbiCompletion::decode(&fixture.handle.read_memory(fixture.rings[1], 32).await?)?;
    assert_eq!(record.command_id(), 11);
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}
