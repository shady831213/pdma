mod irq;
mod queue;

use super::{
    ABI_VERSION, AbiControlError, AbiV1Features, KNOWN_FEATURE_MASK, MAX_INDIRECT_DESTINATIONS,
    MAX_RING_DEPTH, MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES, validate_abi_version,
};

pub use irq::{EpochIrqRoute, EpochIrqSource, IrqControl};
pub use queue::{EpochQueue, QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions};

/// Device generation attached to resettable control capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceEpoch(u64);

impl DeviceEpoch {
    /// Initial generation before the first whole-device reset.
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Creates a generation from a complete fixed-width value.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Complete generation value.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Advances a generation without wrapping.
    pub const fn next(self) -> Result<Self, AbiControlError> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(AbiControlError::DeviceEpochOverflow { current: self.0 }),
        }
    }
}

/// Checked host discovery request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryRequest {
    version: u16,
    required_features: AbiV1Features,
}

impl DiscoveryRequest {
    /// Validates a requested version and required feature mask.
    pub fn try_new(version: u16, required_features: u64) -> Result<Self, AbiControlError> {
        validate_abi_version(version)?;
        let unknown = required_features & !KNOWN_FEATURE_MASK;
        if unknown != 0 {
            return Err(AbiControlError::UnknownFeatureBits { bits: unknown });
        }
        let Ok(required_features) = AbiV1Features::try_from(required_features) else {
            return Err(AbiControlError::UnknownFeatureBits { bits: unknown });
        };
        Ok(Self {
            version,
            required_features,
        })
    }

    /// Requested ABI version.
    pub const fn version(self) -> u16 {
        self.version
    }

    /// Required validated features.
    pub const fn required_features(self) -> AbiV1Features {
        self.required_features
    }
}

/// ABI discovery response for the implemented contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiDiscovery {
    version: u16,
    features: AbiV1Features,
}

impl AbiDiscovery {
    /// Complete ABI-v1 discovery response.
    pub const fn v1() -> Self {
        Self {
            version: ABI_VERSION,
            features: AbiV1Features::all(),
        }
    }

    /// Implemented ABI version.
    pub const fn version(self) -> u16 {
        self.version
    }

    /// Implemented discovery features.
    pub const fn features(self) -> AbiV1Features {
        self.features
    }
}

/// Fixed representational limits advertised by ABI v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiLimits {
    ring_depth: u32,
    indirect_destinations: u32,
    transfer_bytes: u32,
    strided_rows: u32,
    stride: u32,
}

impl AbiLimits {
    /// Canonical ABI-v1 limits.
    pub const fn v1() -> Self {
        Self {
            ring_depth: MAX_RING_DEPTH,
            indirect_destinations: MAX_INDIRECT_DESTINATIONS,
            transfer_bytes: MAX_TRANSFER_BYTES,
            strided_rows: MAX_STRIDED_ROWS,
            stride: MAX_STRIDE,
        }
    }

    /// Maximum ring depth.
    pub const fn max_ring_depth(self) -> u32 {
        self.ring_depth
    }

    /// Maximum indirect destination count.
    pub const fn max_indirect_destinations(self) -> u32 {
        self.indirect_destinations
    }

    /// Maximum direct transfer length.
    pub const fn max_transfer_bytes(self) -> u32 {
        self.transfer_bytes
    }

    /// Maximum strided row count.
    pub const fn max_strided_rows(self) -> u32 {
        self.strided_rows
    }

    /// Maximum stride.
    pub const fn max_stride(self) -> u32 {
        self.stride
    }
}

/// Device execution state returned by typed status discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRunState {
    /// No runnable or pending work.
    Idle,
    /// Work can make progress.
    Running,
    /// Work exists but cannot currently progress.
    Blocked,
}

impl TryFrom<u16> for DeviceRunState {
    type Error = AbiControlError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Idle),
            1 => Ok(Self::Running),
            2 => Ok(Self::Blocked),
            _ => Err(AbiControlError::UnknownStatusValue { value }),
        }
    }
}

/// Whole-device typed status response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStatus {
    epoch: DeviceEpoch,
    run_state: DeviceRunState,
}

impl DeviceStatus {
    /// Creates a status snapshot from already typed values.
    pub const fn new(epoch: DeviceEpoch, run_state: DeviceRunState) -> Self {
        Self { epoch, run_state }
    }

    /// Snapshot generation.
    pub const fn epoch(self) -> DeviceEpoch {
        self.epoch
    }

    /// Snapshot execution state.
    pub const fn run_state(self) -> DeviceRunState {
        self.run_state
    }
}

/// Whole-device reset request guarded by an expected generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WholeDeviceReset {
    expected_epoch: DeviceEpoch,
}

impl WholeDeviceReset {
    /// Creates an epoch-guarded reset request.
    pub const fn new(expected_epoch: DeviceEpoch) -> Self {
        Self { expected_epoch }
    }

    /// Generation that must still be current before reset mutation.
    pub const fn expected_epoch(self) -> DeviceEpoch {
        self.expected_epoch
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
