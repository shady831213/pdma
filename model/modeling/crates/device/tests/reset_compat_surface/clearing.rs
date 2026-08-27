//! Work and retirement clearing fixture.

use std::sync::Arc;

use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, WholeDeviceReset};
use pdma_model_device::compat::SyncDevice;

use super::support::{TestResult, core, queue_config, submit_copy};

pub(super) fn observe() -> TestResult<(bool, bool)> {
    let zero_resources = ResourceConfig::default().with_read_slots(0);
    let mut stalled = SyncDevice::from_core(core(
        Arc::new(SharedMemory::new()),
        zero_resources,
        Arc::new(NoopRegisterObserver),
    ));
    let queue = stalled.create_queue(queue_config())?;
    let input = stalled.allocate_buffer(1)?;
    let output = stalled.allocate_buffer(1)?;
    submit_copy(&mut stalled, queue, input, output, 0)?;
    stalled.run_until_blocked_typed(1)?;
    stalled.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;
    let work_cleared = stalled.run_until_idle_typed(1)?.steps == 0;
    assert_eq!(stalled.resource_config(), zero_resources);

    let mut retiring = SyncDevice::from_core(core(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    let queue = retiring.create_queue(queue_config())?;
    let input = retiring.allocate_buffer(1)?;
    let first = retiring.allocate_buffer(1)?;
    let second = retiring.allocate_buffer(1)?;
    retiring.write_buffer(input, 0, &[9])?;
    submit_copy(&mut retiring, queue, input, first, 0)?;
    submit_copy(&mut retiring, queue, input, second, 1)?;
    retiring.run_until_blocked_typed(2)?;
    retiring.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;
    let retirement_cleared = retiring.run_until_idle_typed(1)?.steps == 0;
    Ok((work_cleared, retirement_cleared))
}
