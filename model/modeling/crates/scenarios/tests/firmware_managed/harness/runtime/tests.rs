//! Managed work-view behavior through real runtime transports.

use hardware_interface_runtime::{RegisterAccess, SharedMemory, SharedMemoryAccess};
use pdma_model_contract::{WINDOW_CONTROL_BASE, WINDOW_STATUS_BASE};
use std::sync::Arc;

use super::{AccessError, Managed};

mod backpressure;
mod boot;
mod client;
mod control;
mod interrupts;
mod multi_channel;
mod notifications;
mod publication;

#[tokio::test]
async fn construction_leaves_host_closed() {
    // Given: an assembled but undriven system.
    let memory = SharedMemoryAccess::from(Arc::new(SharedMemory::new()));
    let (_system, host) = Managed::new(memory).unwrap();
    // When: Host attempts DMA discovery and reads separate readiness.
    let mut value = [0; 4];
    let discovery = host.bus.read(0, &mut value).await;
    host.bus.read(WINDOW_STATUS_BASE, &mut value).await.unwrap();
    // Then: construction did not run firmware or open the hardware window.
    assert!(matches!(discovery, Err(AccessError::Closed)));
    assert_eq!(value, [0; 4]);
}

#[tokio::test]
async fn host_cannot_program_management_before_boot() {
    // Given: the Host work capability, not the firmware management capability.
    let (_system, host) =
        Managed::new(SharedMemoryAccess::from(Arc::new(SharedMemory::new()))).unwrap();
    // When: Host attempts to open the window itself.
    let result = host
        .bus
        .write(WINDOW_CONTROL_BASE, &1u32.to_le_bytes())
        .await;
    // Then: hardware denies the management write.
    assert!(matches!(result, Err(AccessError::Denied)));
}

#[tokio::test]
async fn firmware_ready_does_not_grant_dma_access() {
    // Given: independently running firmware and Host software domains.
    let (system, host) =
        Managed::new(SharedMemoryAccess::from(Arc::new(SharedMemory::new()))).unwrap();
    // When: Host waits for readiness while firmware initializes via management MMIO.
    let result = system
        .run_until(async {
            let mut word = [0; 4];
            for _ in 0..10_000 {
                host.bus
                    .read(pdma_model_contract::FIRMWARE_STATUS_BASE, &mut word)
                    .await
                    .unwrap();
                if u32::from_le_bytes(word) == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(
                u32::from_le_bytes(word),
                1,
                "firmware did not publish readiness"
            );
            assert!(matches!(
                host.bus.read(0, &mut word).await,
                Err(AccessError::Closed)
            ));
            host.bus.read(WINDOW_STATUS_BASE, &mut word).await.unwrap();
            word
        })
        .await
        .unwrap();
    // Then: readiness alone leaves work access closed until a management request.
    assert_eq!(u32::from_le_bytes(result), 0);
}
