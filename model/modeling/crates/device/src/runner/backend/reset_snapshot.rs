use super::DeviceModel;
use crate::runner::bus::control::BusControlState;

impl DeviceModel {
    pub(in crate::runner) fn reset_partition_counts(
        &self,
        control: &BusControlState,
    ) -> (usize, usize, usize, usize, usize) {
        match self {
            Self::QueuePort(device) => device.reset_partition_counts(control),
        }
    }
}
