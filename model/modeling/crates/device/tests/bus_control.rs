//! Atomic DMA queue-control integration tests.

#[path = "bus_control/support.rs"]
/// Shared typed queue-control fixtures.
pub mod support;

use pdma_model_contract::semantic::{
    BufferId, DeviceEpoch, EpochQueue, QueueActivation, QueueControl, QueueCursorUpdate, QueueId,
    QueueRingObjects, RingCursor,
};
use pdma_model_device::{DmaWrite, MappedDmaRegion, PdmaControlError, SubmissionAcceptance};
use support::{activation, new_handle};

include!("bus_control/happy.rs");
include!("bus_control/failure.rs");
include!("bus_control/activation.rs");
include!("bus_control/activation_reset.rs");
