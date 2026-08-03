//! Atomic DMA queue-control integration tests.

#[path = "bus_control/support.rs"]
/// Shared typed queue-control fixtures.
pub mod support;

use pdma_model_contract::abi::{
    DmaAddress, DmaRegion, EpochQueue, QueueActivation, QueueControl, QueueCursorUpdate,
    QueueRingRegions, RingCursor, RingLayout, RingRecord,
};
use pdma_model_contract::{BufferId, DeviceEpoch, QueueId};
use pdma_model_device::{BusControlError, DmaWrite, SubmissionAcceptance};
use support::{activation, new_handle};

include!("bus_control/happy.rs");
include!("bus_control/failure.rs");
include!("bus_control/activation.rs");
include!("bus_control/activation_reset.rs");
