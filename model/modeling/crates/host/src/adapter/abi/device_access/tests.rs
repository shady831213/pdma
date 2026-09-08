use super::AsyncDeviceAccess;
use crate::adapter::abi::{PdmaInterruptReady, PdmaInterruptSourceError, PdmaRegisterAccessError};
use hardware_interface_runtime::{InterruptSource, RegisterAccess, SharedMemory};
use pdma_model_contract::abi::registers::GlobalRegisters;
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, PdmaMemoryProvisioning};
use std::{future::Future, pin::pin, sync::Arc, task::Poll};

fn fixture() -> (AsyncDeviceHandle, AsyncDeviceAccess) {
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let access = AsyncDeviceAccess::new(handle.clone());
    (handle, access)
}

async fn reset(access: &AsyncDeviceAccess, handle: &AsyncDeviceHandle) -> DeviceEpoch {
    access
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("bound reset");
    let mut bytes = [0; 8];
    handle
        .register_endpoint()
        .read(
            u32::try_from(GlobalRegisters::EPOCH.offset()).expect("epoch offset"),
            &mut bytes,
        )
        .await
        .expect("epoch discovery");
    DeviceEpoch::new(u64::from_le_bytes(bytes))
}

#[tokio::test(flavor = "current_thread")]
async fn stale_register_write_cannot_reset_the_new_epoch() {
    // Given: a hardware access capability from before reset.
    let (handle, access) = fixture();
    let epoch = reset(&access, &handle).await;
    let offset = u32::try_from(GlobalRegisters::RESET.offset()).expect("register offset");

    // When: the stale capability attempts another hardware reset.
    let result = access.write(offset, &1_u32.to_le_bytes()).await;

    // Then: epoch rejection precedes mutation, and an explicitly rebound capability still reads epoch 1.
    assert_eq!(
        result,
        Err(PdmaRegisterAccessError::StaleEpoch {
            expected: epoch,
            actual: DeviceEpoch::initial()
        })
    );
    let mut bytes = [0; 8];
    access
        .at_epoch(epoch)
        .read(
            u32::try_from(GlobalRegisters::EPOCH.offset()).expect("epoch offset"),
            &mut bytes,
        )
        .await
        .expect("bound epoch read");
    assert_eq!(u64::from_le_bytes(bytes), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn stale_hardware_irq_claim_returns_epoch_error() {
    // Given: a hardware IRQ output capability from before reset.
    let (handle, mut access) = fixture();
    let epoch = reset(&access, &handle).await;

    // When: that capability attempts to consume the new epoch's hardware output.
    let result = access
        .claim(PdmaInterruptReady::new(DeviceEpoch::initial()))
        .await;

    // Then: it reports the stale capability even though the new output is empty.
    assert_eq!(
        result,
        Err(PdmaInterruptSourceError::StaleEpoch {
            expected: epoch,
            actual: DeviceEpoch::initial()
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn pending_hardware_irq_readiness_wakes_with_epoch_error_after_reset() {
    // Given: an empty hardware output with a pending non-consuming waiter.
    let (handle, access) = fixture();
    let mut ready = pin!(access.wait_ready());
    std::future::poll_fn(|context| {
        assert!(ready.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;

    // When: reset invalidates the waiting capability.
    let epoch = reset(&access, &handle).await;

    // Then: the waiter terminates with an epoch error instead of waiting for new-epoch traffic.
    assert_eq!(
        ready.await,
        Err(PdmaInterruptSourceError::StaleEpoch {
            expected: epoch,
            actual: DeviceEpoch::initial()
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn empty_hardware_irq_claim_returns_without_waiting_for_readiness() {
    // Given: a hardware IRQ output with no notification.
    let (_handle, mut access) = fixture();
    let mut claim = pin!(access.claim(PdmaInterruptReady::new(DeviceEpoch::initial())));

    // When: the consuming operation is polled once.
    let result = std::future::poll_fn(|context| Poll::Ready(claim.as_mut().poll(context))).await;

    // Then: empty output completes immediately without issuing a software command.
    assert_eq!(result, Poll::Ready(Ok(None)));
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_cannot_cross_an_explicit_epoch_rebinding() {
    // Given: readiness observed in the epoch preceding an explicit rebind.
    let (handle, access) = fixture();
    let ready = PdmaInterruptReady::new(DeviceEpoch::initial());
    let epoch = reset(&access, &handle).await;
    let mut access = access.at_epoch(epoch);

    // When: the rebound source receives the earlier readiness token.
    let result = access.claim(ready).await;

    // Then: it rejects the token before touching the new epoch's interrupt output.
    assert_eq!(
        result,
        Err(PdmaInterruptSourceError::StaleEpoch {
            expected: epoch,
            actual: DeviceEpoch::initial()
        })
    );
}
