use hardware_interface_runtime::{
    NoopRegisterObserver, RegisterObservation, RegisterObserver, SharedMemory,
};
use pdma_model_device::test_support::{
    DeviceStateSnapshot, ResetPartitionSnapshot, async_state_snapshot, device_core_at_epoch,
    device_core_at_epoch_with_mmio_write, reset_partition_snapshot, state_snapshot,
    sync_state_snapshot,
};

fn main() {
    let _memory = SharedMemory::new();
    let observer = NoopRegisterObserver;
    let _: &dyn RegisterObserver = &observer;
    let _: Option<RegisterObservation> = None;
    let _: Option<DeviceStateSnapshot> = None;
    let _: Option<ResetPartitionSnapshot> = None;
    let _ = device_core_at_epoch;
    let _ = device_core_at_epoch_with_mmio_write;
    let _ = reset_partition_snapshot;
    let _ = state_snapshot;
    let _ = async_state_snapshot;
    let _ = sync_state_snapshot;
}
