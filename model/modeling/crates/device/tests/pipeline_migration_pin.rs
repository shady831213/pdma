#![allow(missing_docs)]
#![allow(clippy::expect_used, reason = "checked test fixtures")]

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::registers::{
    GlobalRegisters, IrqSourceRegisters, MmioAccessError, QueueRegisters, irq_offset, queue_offset,
};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, TraceEventKind};
use pdma_model_device::{AsyncStallReason, AsyncStepStatus, MmioRegisterError};

#[expect(
    clippy::redundant_pub_crate,
    reason = "private test module bounds helper visibility without unreachable public APIs"
)]
mod pipeline_migration_pin {
    pub(crate) mod support;
}
use pipeline_migration_pin::support::{
    Fixture, adjacent, count, operands, read, register_read, register_write, wait_idle, write,
};

#[tokio::test(flavor = "current_thread")]
#[rustfmt::skip]
async fn stalled_resume_retries_publication_without_refetch_or_reexecution() {
    // Given: two accepted copies, one completion slot, and an enabled completion IRQ.
    let mut fixture = Fixture::new(ResourceConfig::default().with_completion_publication_slots(1)).await;
    fixture.handle.enable_tracing().await;
    register_write(&fixture.endpoint, irq_offset(0, 0, IrqSourceRegisters::ENABLED.offset()).expect("IRQ offset"), 1).await;
    register_write(&fixture.endpoint, irq_offset(0, 0, IrqSourceRegisters::MASKED.offset()).expect("IRQ offset"), 0).await;
    let [source, first, second] = operands(&fixture.allocator);
    write(&fixture.handle, source, &[1, 3, 5, 7]).await;
    fixture.submit(0, 20, source, first).await;
    fixture.submit(1, 21, source, second).await;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 2).await;
    let before = fixture.handle.snapshot().await;
    let worker = fixture.start();

    // When: command two stalls after DMA, its source changes, and publication space opens.
    let stalled = fixture.handle.wait_for_stall_after(0).await;
    write(&fixture.handle, source, &[9; 4]).await;
    let repeated = fixture.handle.wait_for_stall_after(1).await;
    fixture.cursor(QueueRegisters::CQ_CONSUMER.offset(), 1).await;
    let resumed = fixture.handle.wait_for_progress_after(1).await;
    let cursors = fixture.cursors().await;
    let trace = fixture.handle.trace_events().await;

    // Then: upstream work ran once, retirement alone retried, and visibility stayed ordered.
    assert_eq!((stalled.steps, stalled.stalls, stalled.errors), (1, 1, 0));
    assert_eq!(stalled.generation(), before.generation() + 2);
    assert_eq!((repeated.steps, repeated.stalls), (1, 2));
    assert_eq!(repeated.generation(), stalled.generation() + 2);
    assert_eq!(fixture.handle.native_steps().await, 2);
    assert_eq!(cursors, [2, 2, 1]);
    assert_eq!(read(&fixture.handle, second).await, [1, 3, 5, 7]);
    assert_eq!(count(&trace, TraceEventKind::ExecutionSucceeded), 2);
    assert_eq!(count(&trace, TraceEventKind::CompletionPublished), 2);
    assert_eq!(count(&trace, TraceEventKind::IrqSourceLatched), 2);
    assert!(adjacent(&trace, TraceEventKind::CompletionPublished, TraceEventKind::IrqSourceLatched));
    println!("stalled_resume steps=2 stalls=2 generation={} native_steps=2 cursors=2/2/1 executions=2 publications=2", resumed.generation());
    wait_idle(&fixture.handle).await;
    fixture.handle.request_shutdown().await;
    let summary = worker.await.expect("worker join");
    assert_eq!((summary.steps, summary.stalls, summary.errors), (2, 2, 0));
}

#[tokio::test(flavor = "current_thread")]
#[rustfmt::skip]
async fn reset_pending_blocks_pre_execution_and_post_execution_publication() {
    // Given: one command blocked by execution resources before DMA.
    let mut blocked = Fixture::new(ResourceConfig::default().with_read_slots(0)).await;
    blocked.handle.enable_tracing().await;
    let [source, target, _unused] = operands(&blocked.allocator);
    write(&blocked.handle, source, &[4, 3, 2, 1]).await;
    blocked.submit(0, 30, source, target).await;
    blocked.cursor(QueueRegisters::DOORBELL.offset(), 1).await;
    let memory_revision = blocked.memory.revision();
    let blocked_worker = blocked.start();
    let resource_stall = blocked.handle.wait_for_stall_after(0).await;

    // When: reset advances the epoch at the execution-resource pending boundary.
    let epoch = reset(&blocked).await;

    // Then: admission stalls before fetch, DMA, cursors, IRQ, or success publication.
    assert_eq!(resource_stall.last_status, AsyncStepStatus::Stalled(AsyncStallReason::ExecutionResourcePressure));
    assert_eq!((resource_stall.steps, resource_stall.stalls, resource_stall.errors), (0, 1, 0));
    assert_eq!(blocked.handle.native_steps().await, 0);
    assert_eq!(blocked.memory.revision(), memory_revision);
    assert_reset_quiescent(&blocked, epoch, &[target], (vec![vec![0; 4]], [0, 0, 0], [0; 2], false)).await;
    blocked.handle.request_shutdown().await;
    let blocked_summary = blocked_worker.await.expect("blocked worker join");
    assert_eq!((blocked_summary.steps, blocked_summary.stalls, blocked_summary.errors), (0, 1, 0));

    // Given: a second command whose DMA executed before completion publication stalled.
    let mut pending = Fixture::new(ResourceConfig::default().with_completion_publication_slots(1)).await;
    pending.handle.enable_tracing().await;
    let [source, first, second] = operands(&pending.allocator);
    write(&pending.handle, source, &[0xa5, 0x5a, 0x33, 0xcc]).await;
    pending.submit(0, 40, source, first).await;
    pending.submit(1, 41, source, second).await;
    pending.cursor(QueueRegisters::DOORBELL.offset(), 2).await;
    let pending_worker = pending.start();
    let publication_stall = pending.handle.wait_for_stall_after(0).await;
    assert_eq!(pending.cursors().await, [2, 1, 0]);
    assert_eq!(read(&pending.handle, second).await, [0xa5, 0x5a, 0x33, 0xcc]);
    let generation = pending.memory.revision();

    // When: reset drops the post-execution pending retirement.
    let epoch = reset(&pending).await;

    // Then: both DMA writes persist, but only the first completion was ever published.
    assert_eq!((publication_stall.steps, publication_stall.stalls, publication_stall.errors), (1, 1, 0));
    assert_eq!(pending.handle.native_steps().await, 2);
    assert_eq!(pending.memory.revision(), generation);
    let bytes = vec![vec![0xa5, 0x5a, 0x33, 0xcc]; 2];
    assert_reset_quiescent(&pending, epoch, &[first, second], (bytes, [2, 1, 0], [0; 2], false)).await;
    println!("reset_pending pre_fetch=0 pre_dma=0 post_fetch=2 post_dma=2 publications=1 steps=1 stalls=1 epoch=1 post_idle_stable=true fresh_epoch_result_empty=true");
    pending.handle.request_shutdown().await;
    let summary = pending_worker.await.expect("pending worker join");
    assert_eq!((summary.steps, summary.stalls, summary.errors), (1, 1, 0));
}

async fn reset(fixture: &Fixture) -> DeviceEpoch {
    register_write(
        &fixture.endpoint,
        u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
        1,
    )
    .await;
    DeviceEpoch::new(
        register_read(
            &fixture.endpoint,
            u32::try_from(GlobalRegisters::EPOCH.offset()).expect("epoch offset"),
        )
        .await,
    )
}

type ResetObservables = (Vec<Vec<u8>>, [usize; 3], [u64; 2], bool);

#[rustfmt::skip]
async fn reset_observables(fixture: &Fixture, addresses: &[DmaAddress]) -> ResetObservables {
    let mut bytes = Vec::with_capacity(addresses.len());
    for &address in addresses { bytes.push(read(&fixture.handle, address).await); }
    let trace = fixture.handle.trace_events().await;
    let counts = [count(&trace, TraceEventKind::ExecutionSucceeded), count(&trace, TraceEventKind::CompletionPublished), count(&trace, TraceEventKind::IrqSourceLatched)];
    let producers = [register_read(&fixture.endpoint, queue_offset(0, QueueRegisters::CQ_PRODUCER.offset()).expect("CQ offset")).await,
        register_read(&fixture.endpoint, queue_offset(0, QueueRegisters::EQ_PRODUCER.offset()).expect("EQ offset")).await];
    (bytes, counts, producers, fixture.handle.irq_notification_ready().await)
}

#[rustfmt::skip]
async fn assert_reset_quiescent(fixture: &Fixture, epoch: DeviceEpoch, addresses: &[DmaAddress], expected: ResetObservables) {
    let irq_status = irq_offset(0, 0, IrqSourceRegisters::STATUS.offset()).expect("IRQ offset");
    assert_eq!(fixture.endpoint.read(irq_status, &mut [0; 4]).await,
        Err(MmioRegisterError::Access(MmioAccessError::ReservedOffset { offset: irq_status })));
    let at_reset = reset_observables(fixture, addresses).await;
    assert_eq!(at_reset, expected);
    wait_idle(&fixture.handle).await;
    assert_eq!(reset_observables(fixture, addresses).await, at_reset);
    assert_eq!(fixture.endpoint.write(queue_offset(0, QueueRegisters::ENABLE.offset()).expect("enable offset"), &1_u32.to_le_bytes()).await,
        Err(MmioRegisterError::StaleEpoch { expected: epoch, actual: DeviceEpoch::initial() }));
    fixture.activate(&fixture.endpoint.at_epoch(epoch)).await;
    assert_eq!(fixture.cursors().await, [0; 3]);
    wait_idle(&fixture.handle).await;
    assert_eq!(reset_observables(fixture, addresses).await, at_reset);
}
