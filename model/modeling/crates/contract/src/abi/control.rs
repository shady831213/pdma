use super::{
    ABI_VERSION, AbiControlError, AbiV1Features, KNOWN_FEATURE_MASK, MAX_INDIRECT_DESTINATIONS,
    MAX_RING_DEPTH, MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES, validate_abi_version,
};

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
