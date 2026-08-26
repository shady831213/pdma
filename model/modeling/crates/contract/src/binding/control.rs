mod queue;

use crate::abi::{AbiControlError, DiscoveryRequest};

pub use queue::{QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions};

use crate::semantic::{DeviceEpoch, IrqControl, WholeDeviceReset};

impl DeviceEpoch {
    /// Advances a generation without wrapping.
    pub const fn next(self) -> Result<Self, AbiControlError> {
        match self.raw().checked_add(1) {
            Some(next) => Ok(Self::new(next)),
            None => Err(AbiControlError::DeviceEpochOverflow {
                current: self.raw(),
            }),
        }
    }
}

/// Closed typed control request vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRequest {
    /// Discover version and feature compatibility.
    Discover(DiscoveryRequest),
    /// Read fixed ABI limits.
    ReadLimits,
    /// Read whole-device status.
    ReadStatus,
    /// Atomically activate one queue.
    ActivateQueue(QueueActivation),
    /// Operate on queue-local cursor or status state.
    Queue(QueueControl),
    /// Operate on one PDMA-local IRQ source.
    Irq(IrqControl),
    /// Reset all resettable device state.
    Reset(WholeDeviceReset),
}
