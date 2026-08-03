use crate::abi::mmio::GlobalCommand;

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
    pub fn to_le_bytes(self) -> [u8; 44] {
        let mut bytes = [0; 44];
        let values = self
            .abi_version
            .to_le_bytes()
            .into_iter()
            .chain(self.run_state.to_le_bytes())
            .chain(self.features.to_le_bytes())
            .chain(self.epoch.to_le_bytes())
            .chain(self.max_ring_depth.to_le_bytes())
            .chain(self.max_indirect_destinations.to_le_bytes())
            .chain(self.max_transfer_bytes.to_le_bytes())
            .chain(self.max_strided_rows.to_le_bytes())
            .chain(self.max_stride.to_le_bytes());
        for (destination, source) in bytes.iter_mut().zip(values) {
            *destination = source;
        }
        bytes
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
