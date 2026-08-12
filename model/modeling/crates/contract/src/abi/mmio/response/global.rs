use hardware_interface_schema::{RegisterCodecError, register_map};

use super::ResponseCodecError;
use crate::abi::mmio::{GlobalCommand, global};

register_map! { pub map GlobalResponseRegisters {
    size: 44,
    fixed: {
        abi_version: global::GRspAbiVersion { id: 0, name: "G_RSP_ABI_VERSION", offset: 0 },
        run_state: global::GRspRunState { id: 1, name: "G_RSP_RUN_STATE", offset: 4 },
        features: global::GRspFeatures { id: 2, name: "G_RSP_FEATURES", offset: 8 },
        epoch: global::GRspEpoch { id: 3, name: "G_RSP_EPOCH", offset: 16 },
        max_ring_depth: global::GMaxRingDepth { id: 4, name: "G_MAX_RING_DEPTH", offset: 24 },
        max_indirect_destinations: global::GMaxIndirectDestinations { id: 5, name: "G_MAX_INDIRECT_DESTINATIONS", offset: 28 },
        max_transfer_bytes: global::GMaxTransferBytes { id: 6, name: "G_MAX_TRANSFER_BYTES", offset: 32 },
        max_strided_rows: global::GMaxStridedRows { id: 7, name: "G_MAX_STRIDED_ROWS", offset: 36 },
        max_stride: global::GMaxStride { id: 8, name: "G_MAX_STRIDE", offset: 40 },
    },
    banks: {}
} }

/// Exact bytes published through the global response registers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlobalResponseBank {
    abi_version: u32,
    run_state: u32,
    features: u64,
    epoch: u64,
    max_ring_depth: u32,
    max_indirect_destinations: u32,
    max_transfer_bytes: u32,
    max_strided_rows: u32,
    max_stride: u32,
}

impl GlobalResponseBank {
    /// Exact contiguous little-endian response-register bytes.
    pub fn to_le_bytes(self) -> Result<[u8; 44], ResponseCodecError> {
        let mut bytes = [0; 44];
        let mut view = GlobalResponseRegistersViewMut::new(&mut bytes)?;
        self.write_to(&mut view)?;
        Ok(bytes)
    }

    fn write_to(
        self,
        view: &mut GlobalResponseRegistersViewMut<'_>,
    ) -> Result<(), RegisterCodecError> {
        view.abi_version_mut()?
            .set_value(u64::from(self.abi_version))?;
        view.run_state_mut()?.set_value(u64::from(self.run_state))?;
        view.features_mut()?.set_value(self.features)?;
        view.epoch_mut()?.set_value(self.epoch)?;
        view.max_ring_depth_mut()?
            .set_value(u64::from(self.max_ring_depth))?;
        view.max_indirect_destinations_mut()?
            .set_value(u64::from(self.max_indirect_destinations))?;
        view.max_transfer_bytes_mut()?
            .set_value(u64::from(self.max_transfer_bytes))?;
        view.max_strided_rows_mut()?
            .set_value(u64::from(self.max_strided_rows))?;
        view.max_stride_mut()?.set_value(u64::from(self.max_stride))
    }
}

/// Global limits returned by `READ_LIMITS`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlobalLimitsResponse {
    /// Maximum supported ring depth.
    pub max_ring_depth: u32,
    /// Maximum indirect broadcast destinations.
    pub max_indirect_destinations: u32,
    /// Maximum transfer extent in bytes.
    pub max_transfer_bytes: u32,
    /// Maximum supported strided rows.
    pub max_strided_rows: u32,
    /// Maximum supported stride.
    pub max_stride: u32,
}

/// Global run state returned by `READ_STATUS` or `RESET`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlobalStatusResponse {
    /// Numeric device run state.
    pub run_state: u32,
    /// Current device epoch.
    pub epoch: u64,
}

/// Post-reset device epoch returned by `RESET`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlobalResetResponse {
    /// Current device epoch after the successful reset.
    pub epoch: u64,
}

/// Closed successful global command responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalResponse {
    /// `DISCOVER` returns ABI version and negotiated features.
    Discover {
        /// MMIO ABI version.
        abi_version: u32,
        /// Supported feature bits.
        features: u64,
    },
    /// `READ_LIMITS` returns the global limits block.
    ReadLimits(GlobalLimitsResponse),
    /// `READ_STATUS` returns current run state and epoch.
    ReadStatus(GlobalStatusResponse),
    /// `RESET` returns only the post-reset epoch.
    Reset(GlobalResetResponse),
}

impl GlobalResponse {
    /// Command that owns this closed response shape.
    pub const fn command(self) -> GlobalCommand {
        match self {
            Self::Discover { .. } => GlobalCommand::Discover,
            Self::ReadLimits(_) => GlobalCommand::ReadLimits,
            Self::ReadStatus(_) => GlobalCommand::ReadStatus,
            Self::Reset(_) => GlobalCommand::Reset,
        }
    }

    /// Populates the exact response bank, zero-filling irrelevant fields.
    pub const fn bank(self) -> GlobalResponseBank {
        match self {
            Self::Discover {
                abi_version,
                features,
            } => GlobalResponseBank {
                abi_version,
                features,
                ..GlobalResponseBank::zero()
            },
            Self::ReadLimits(response) => GlobalResponseBank {
                max_ring_depth: response.max_ring_depth,
                max_indirect_destinations: response.max_indirect_destinations,
                max_transfer_bytes: response.max_transfer_bytes,
                max_strided_rows: response.max_strided_rows,
                max_stride: response.max_stride,
                ..GlobalResponseBank::zero()
            },
            Self::ReadStatus(response) => GlobalResponseBank {
                run_state: response.run_state,
                epoch: response.epoch,
                ..GlobalResponseBank::zero()
            },
            Self::Reset(response) => GlobalResponseBank {
                epoch: response.epoch,
                ..GlobalResponseBank::zero()
            },
        }
    }
}

impl GlobalResponseBank {
    const fn zero() -> Self {
        Self {
            abi_version: 0,
            run_state: 0,
            features: 0,
            epoch: 0,
            max_ring_depth: 0,
            max_indirect_destinations: 0,
            max_transfer_bytes: 0,
            max_strided_rows: 0,
            max_stride: 0,
        }
    }
}

/// Global response-bank effect of a structurally valid command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalResponseUpdate {
    /// Successful command response replaces the bank.
    Success(GlobalResponse),
    /// Semantic failure preserves the preceding bank.
    Preserve,
}

/// Applies the global response-bank success or failure rule.
pub const fn apply_global_response_update(
    previous: GlobalResponseBank,
    update: GlobalResponseUpdate,
) -> GlobalResponseBank {
    match update {
        GlobalResponseUpdate::Success(response) => response.bank(),
        GlobalResponseUpdate::Preserve => previous,
    }
}
