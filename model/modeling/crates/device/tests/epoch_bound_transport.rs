//! Hardware-facing register commit boundary regressions.

use hardware_interface_runtime::{RegisterAccess, RegisterBusBase, SharedMemory};
use pdma_model_contract::abi::registers::{GlobalRegisters, QueueRegisters, queue_offset};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::{AsyncDevice, MmioRegisterError, PdmaMemoryProvisioning};
use std::sync::Arc;

#[tokio::test]
async fn old_binding_rejects_writes_while_direct_epoch_discovery_allows_recovery() {
    // Given: an addressed endpoint retained across a committed whole-device reset.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let stale = device
        .handle()
        .register_endpoint()
        .at_mmio_base(RegisterBusBase::new(0x8000_0000));
    stale
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("reset");
    let mut epoch = [0; 8];
    stale
        .read(
            u32::try_from(GlobalRegisters::EPOCH.offset()).expect("epoch offset"),
            &mut epoch,
        )
        .await
        .expect("epoch discovery");
    let current = DeviceEpoch::new(u64::from_le_bytes(epoch));
    let fresh = stale.at_epoch(current);
    let base = queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("base");
    fresh
        .write(base, &17_u64.to_le_bytes())
        .await
        .expect("fresh configuration");

    // When: the old endpoint attempts to overwrite current-epoch configuration.
    let rejected = stale.write(base, &99_u64.to_le_bytes()).await;

    // Then: the stale write fails before mutation and rebinding preserves addressed geometry.
    assert_eq!(
        rejected,
        Err(MmioRegisterError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial()
        })
    );
    let mut value = [0; 8];
    fresh
        .read(base, &mut value)
        .await
        .expect("current configuration");
    assert_eq!(u64::from_le_bytes(value), 17);
    assert_eq!(fresh.mmio_base(), stale.mmio_base());
    assert_eq!(fresh.epoch(), current);
}

#[tokio::test]
async fn rejected_hardware_transition_preserves_gate_for_next_valid_write() {
    // Given: a queue without the DMA configuration required to enable it.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let endpoint = device.handle().register_endpoint();

    // When: enable rejects and a later independent configuration write is valid.
    let rejected = endpoint
        .write(
            queue_offset(0, QueueRegisters::ENABLE.offset()).expect("enable"),
            &1_u32.to_le_bytes(),
        )
        .await;
    let valid = endpoint
        .write(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("base"),
            &17_u64.to_le_bytes(),
        )
        .await;

    // Then: the rejected operation did not poison or consume an observation slot.
    assert!(matches!(rejected, Err(MmioRegisterError::Control(_))));
    assert_eq!(valid, Ok(()));
    let mut value = [0; 8];
    endpoint
        .read(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("base"),
            &mut value,
        )
        .await
        .expect("read accepted value");
    assert_eq!(u64::from_le_bytes(value), 17);
}
