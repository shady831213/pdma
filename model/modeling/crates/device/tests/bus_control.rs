//! Atomic DMA queue-control integration tests.

#[path = "bus_control/support.rs"]
/// Shared typed queue-control fixtures.
pub mod support;

use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::{RingCursor, RingRecord};
use pdma_model_contract::bridge::{
    PdmaMemoryRegion, QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions,
    RingLayout,
};
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{BufferId, DeviceEpoch, QueueId};
use pdma_model_device::{BusControlError, DmaWrite, SubmissionAcceptance};
use support::{activation, new_handle};

include!("bus_control/happy.rs");
include!("bus_control/failure.rs");
include!("bus_control/activation.rs");
include!("bus_control/activation_reset.rs");
