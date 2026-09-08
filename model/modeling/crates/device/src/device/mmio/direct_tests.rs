use crate::DeviceCore;
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::registers::{GlobalRegisters, locate};
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::Arc;

fn register(offset: usize) -> pdma_model_contract::abi::registers::RegisterLocation {
    locate(u32::try_from(offset).expect("offset")).expect("register")
}

#[test]
fn direct_epoch_read_observes_reset_without_response_command() {
    // Given: a hardware owner with direct generated register addresses.
    let mut core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    let epoch = locate(u32::try_from(GlobalRegisters::EPOCH.offset()).expect("offset"))
        .expect("epoch register");
    let reset = locate(u32::try_from(GlobalRegisters::RESET.offset()).expect("offset"))
        .expect("reset register");
    // When: the reset action is written once.
    core.write_mmio_register(reset, 1).expect("reset");
    // Then: the ordinary read observes the live hardware epoch.
    assert_eq!(
        core.read_register(epoch).expect("epoch"),
        1_u64.to_le_bytes()
    );
}

#[test]
fn read_only_global_write_preserves_hardware_state() {
    // Given: a fresh direct hardware owner.
    let mut core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    let epoch = register(GlobalRegisters::EPOCH.offset());
    let before = core.read_register(epoch);
    // When: software tries to write the hardware epoch.
    let result = core.write_mmio_register(epoch, 9);
    // Then: the write is rejected without changing the epoch.
    assert!(result.is_err());
    assert_eq!(core.read_register(epoch), before);
    assert_eq!(core.reset_runner_calls(), 0);
}

#[test]
fn epoch_overflow_preserves_hardware_and_reset_accounting() {
    // Given: hardware already at the last representable reset epoch.
    let mut core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    core.reset(pdma_model_contract::semantic::DeviceEpoch::new(u64::MAX));
    let before = core.global_values();
    // When: software requests another whole-device reset.
    let result = core.write_mmio_register(register(GlobalRegisters::RESET.offset()), 1);
    // Then: overflow prevents every reset mutation.
    assert!(result.is_err());
    assert_eq!(core.global_values(), before);
    assert_eq!(core.reset_runner_calls(), 0);
}

#[test]
fn disable_zero_is_inert_for_an_inactive_queue() {
    // Given: the next hardware queue is inactive.
    let mut core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    let offset = pdma_model_contract::abi::registers::queue_offset(
        0,
        pdma_model_contract::abi::registers::QueueRegisters::ENABLE.offset(),
    )
    .expect("offset");
    // When: software writes the inactive enable value.
    let changed = core.write_mmio_register(locate(offset).expect("register"), 0);
    // Then: no queue is activated or allocated.
    assert_eq!(changed, Ok(false));
    assert_eq!(core.queues.queue_count(), 0);
}

#[test]
fn frontend_fault_distinguishes_first_publication_attempt_from_retry() {
    // Given: a malformed descriptor version and no event publication capacity.
    let mut provisioning = crate::PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new()));
    let mut core = DeviceCore::new(
        provisioning.memory_access(),
        ResourceConfig::default().with_event_publication_slots(0),
    );
    let queue = crate::test_support::configure(&mut core, &mut provisioning, 0, [2, 2, 2]);
    crate::test_support::submit(
        &mut core,
        queue,
        crate::test_support::copy(7, 0xffff_0000, 0xffff_0040, 4),
    );
    core.memory
        .write_memory(queue.layouts.submission().region().base(), &[2, 0])
        .expect("invalid version");
    crate::test_support::doorbell(&mut core, queue);
    assert_eq!(core.step(), Ok(crate::device::ModelStepStatus::FaultCached));
    // When: the retained frontend fault first attempts publication, then retries.
    let first = core.step();
    let retry = core.step();
    // Then: the stall reports actual publication history instead of decoder origin.
    assert_eq!(
        first,
        Ok(crate::device::ModelStepStatus::Stalled(
            crate::AsyncStallReason::EventPublicationPressure
        ))
    );
    assert_eq!(
        retry,
        Ok(crate::device::ModelStepStatus::Stalled(
            crate::AsyncStallReason::FaultRetirementPendingRetry
        ))
    );
}
