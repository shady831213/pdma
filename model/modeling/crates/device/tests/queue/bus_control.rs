//! Atomic DMA queue-control integration tests.

/// Shared typed queue-control fixtures.
mod support;

mod activation;
mod activation_reset;
mod failure;
mod happy;

use pdma_model_contract::semantic::{
    BufferId, DeviceEpoch, EpochQueue, QueueActivation, QueueControl, QueueCursorUpdate, QueueId,
    QueueRingObjects, RingCursor,
};
use pdma_model_device::{DmaWrite, MappedDmaRegion, PdmaControlError, SubmissionAcceptance};
use support::{activation, new_handle};
