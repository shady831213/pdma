use crate::test_support::{configure, copy, doorbell, irq_write, submit};
use crate::{AsyncDevice, AsyncDeviceHandle, DeviceCore, ModelError, PdmaMemoryProvisioning};
use hardware_interface_runtime::{RegisterAccess, SharedMemory};
use pdma_model_contract::abi::registers::{GlobalRegisters, IrqSourceRegisters};
use pdma_model_contract::semantic::{DeviceEpoch, IrqNotification, ResourceConfig};
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Waker},
};

mod wake;

fn publish_message(core: &mut DeviceCore, provisioning: &mut PdmaMemoryProvisioning) {
    let queue = configure(core, provisioning, 0, [4, 4, 4]);
    let source = provisioning
        .allocate(&mut core.memory, 4)
        .expect("source memory");
    let destination = provisioning
        .allocate(&mut core.memory, 4)
        .expect("destination memory");
    core.memory
        .write_memory(source.base(), &[2, 4, 6, 8])
        .expect("source write");
    core.enable_tracing();
    for (offset, value) in [
        (IrqSourceRegisters::ROUTE_KIND.offset(), 1),
        (IrqSourceRegisters::ROUTE_VECTOR.offset(), 3),
        (IrqSourceRegisters::ENABLED.offset(), 1),
        (IrqSourceRegisters::MASKED.offset(), 0),
    ] {
        irq_write(core, queue, 0, offset, value);
    }
    submit(
        core,
        queue,
        copy(1, source.base().raw(), destination.base().raw(), 4),
    );
    doorbell(core, queue);
    core.step().expect("copy retires");
    assert_eq!(
        core.memory
            .read_memory(destination.base(), 4)
            .expect("copied memory"),
        [2, 4, 6, 8]
    );
}

async fn pending_message() -> AsyncDeviceHandle {
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    handle
        .shared
        .backend
        .interact(|state| publish_message(&mut state.core, &mut state.provisioning))
        .await;
    handle
}

#[tokio::test]
async fn atomic_take_returns_empty_after_another_consumer_wins_readiness_race() {
    // Given: readiness observed twice for one actual hardware message.
    let handle = pending_message().await;
    assert!(handle.irq_notification_ready().await);
    assert!(
        handle
            .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
            .await
            .expect("current epoch")
    );
    let first = handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await
        .expect("first take");

    // When: a second consumer claims after the first has removed the message.
    let second = handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await;

    // Then: the operation completes with Empty and never waits for a future message.
    assert!(matches!(first, Some(IrqNotification::Message(message)) if message.id().raw() == 0));
    assert_eq!(second, Ok(None));
    assert!(!handle.irq_notification_ready().await);
    assert_eq!(
        handle
            .trace_events()
            .await
            .iter()
            .filter(|event| event.kind()
                == pdma_model_contract::semantic::TraceEventKind::IrqMessageEmitted)
            .count(),
        1
    );
}

#[tokio::test]
async fn stale_take_cannot_consume_current_epoch_notification() {
    // Given: a message in a new device epoch.
    let handle = pending_message().await;
    handle
        .register_endpoint()
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("reset");
    let current = DeviceEpoch::new(1);
    handle
        .shared
        .backend
        .interact(|state| publish_message(&mut state.core, &mut state.provisioning))
        .await;

    // When: a stale capability attempts to take the new message.
    let stale = handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await;

    // Then: the current notification survives the typed stale rejection.
    assert_eq!(
        stale,
        Err(ModelError::StaleDeviceEpoch {
            expected: current,
            actual: DeviceEpoch::initial()
        })
    );
    assert!(
        handle
            .take_irq_notification_at_epoch(current)
            .await
            .expect("current take")
            .is_some()
    );
}

#[tokio::test]
async fn pending_take_cancellation_preserves_notification() {
    // Given: a pending message and a backend lock held by a synchronized actor.
    let handle = pending_message().await;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let shared = Arc::clone(&handle.shared);
    let holder = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("holder runtime")
            .block_on(shared.backend.hold_for_test(ready_tx, release_rx));
    });
    ready_rx.recv().expect("holder started");

    // When: a take waits for the lock and is dropped before its commit point.
    let mut pending = Box::pin(handle.take_irq_notification_at_epoch(DeviceEpoch::initial()));
    assert!(matches!(
        pending
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(pending);
    release_tx.send(()).expect("release holder");
    holder.join().expect("holder joins");

    // Then: the first message identity remains available to the next take.
    let notification = handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await
        .expect("take");
    assert!(
        matches!(notification, Some(IrqNotification::Message(message)) if message.id().raw() == 0)
    );
}

#[tokio::test]
async fn readiness_wait_rejects_epoch_changed_while_pending() {
    // Given: a drained device and a readiness wait suspended in the initial epoch.
    let handle = pending_message().await;
    handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await
        .expect("drain");
    let mut waiting = Box::pin(handle.wait_irq_notification_ready_at_epoch(DeviceEpoch::initial()));
    assert!(matches!(
        waiting
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));

    // When: reset wakes the suspended readiness observer.
    handle
        .register_endpoint()
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("reset");
    let current = DeviceEpoch::new(1);

    // Then: it returns stale instead of waiting forever on a new generation.
    assert_eq!(
        waiting.await,
        Err(ModelError::StaleDeviceEpoch {
            expected: current,
            actual: DeviceEpoch::initial()
        })
    );
}
