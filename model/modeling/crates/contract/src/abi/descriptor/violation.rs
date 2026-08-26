mod display;

use hardware_interface_schema::SchemaLocation;

use crate::abi::{
    ABI_VERSION, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE, DescriptorHeaderSchema,
    DescriptorHeaderSchemaOpcode, DescriptorSchema, DescriptorSchemaPayloadBroadcastCopyValues,
    DmaAddress, INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS, MAX_TRANSFER_BYTES,
    VECTOR_ADD_OPCODE,
};

use super::error::DescriptorPayloadViolation;
use super::location_offset;

/// Typed hardware validation class for one malformed descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorViolationKind {
    /// One generated descriptor-header field location.
    Header(SchemaLocation),
    /// One operation-specific required-zero storage location.
    Reserved {
        /// Generated descriptor operation.
        opcode: DescriptorHeaderSchemaOpcode,
        /// Generated schema location of the nonzero storage.
        location: SchemaLocation,
    },
    /// One generated descriptor scalar-field location.
    Scalar(SchemaLocation),
}

/// A hardware-visible malformed-descriptor condition produced by ABI decoding.
///
/// Direct construction is intentionally unavailable: only the checked descriptor decoder can
/// create values, so conversion to an [`crate::abi::AbiFault`] cannot fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorViolation(DescriptorViolationValue);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::abi) enum DescriptorViolationValue {
    InvalidVersion {
        observed: u16,
    },
    InvalidSize {
        observed: u16,
    },
    InvalidOpcode {
        observed: u16,
    },
    InvalidFlags {
        observed: u16,
    },
    Reserved {
        opcode: DescriptorHeaderSchemaOpcode,
        location: SchemaLocation,
        observed: u64,
    },
    Scalar {
        location: SchemaLocation,
        observed: u64,
    },
    IndirectCount {
        list: DmaAddress,
        observed: u64,
    },
    IndirectAlignment {
        list: DmaAddress,
    },
}

impl DescriptorViolation {
    pub(in crate::abi) const fn value(self) -> DescriptorViolationValue {
        self.0
    }

    pub(super) const fn version(observed: u16) -> Self {
        Self(DescriptorViolationValue::InvalidVersion { observed })
    }

    pub(super) const fn size(observed: u16) -> Self {
        Self(DescriptorViolationValue::InvalidSize { observed })
    }

    pub(super) const fn opcode(observed: u16) -> Self {
        Self(DescriptorViolationValue::InvalidOpcode { observed })
    }

    pub(super) const fn flags(observed: u16) -> Self {
        Self(DescriptorViolationValue::InvalidFlags { observed })
    }

    pub(super) const fn reserved(
        opcode: DescriptorHeaderSchemaOpcode,
        location: SchemaLocation,
        observed: u64,
    ) -> Self {
        Self(DescriptorViolationValue::Reserved {
            opcode,
            location,
            observed,
        })
    }

    pub(super) const fn scalar(location: SchemaLocation, observed: u64) -> Self {
        Self(DescriptorViolationValue::Scalar { location, observed })
    }

    pub(super) const fn indirect_count(list: DmaAddress, observed: u64) -> Self {
        Self(DescriptorViolationValue::IndirectCount { list, observed })
    }

    pub(super) const fn indirect_alignment(list: DmaAddress) -> Self {
        Self(DescriptorViolationValue::IndirectAlignment { list })
    }

    /// Returns the generated schema location and validation class.
    pub const fn kind(self) -> DescriptorViolationKind {
        match self.0 {
            DescriptorViolationValue::InvalidVersion { .. } => DescriptorViolationKind::Header(
                DescriptorHeaderSchema::VERSION_IDENTITY
                    .at_byte(DescriptorHeaderSchema::VERSION_OFFSET),
            ),
            DescriptorViolationValue::InvalidSize { .. } => DescriptorViolationKind::Header(
                DescriptorHeaderSchema::SIZE_IDENTITY.at_byte(DescriptorHeaderSchema::SIZE_OFFSET),
            ),
            DescriptorViolationValue::InvalidOpcode { .. } => DescriptorViolationKind::Header(
                DescriptorHeaderSchema::OPCODE_IDENTITY
                    .at_byte(DescriptorHeaderSchema::OPCODE_OFFSET),
            ),
            DescriptorViolationValue::InvalidFlags { .. } => DescriptorViolationKind::Header(
                DescriptorHeaderSchema::FLAGS_IDENTITY
                    .at_byte(DescriptorHeaderSchema::FLAGS_OFFSET),
            ),
            DescriptorViolationValue::Reserved {
                opcode, location, ..
            } => DescriptorViolationKind::Reserved { opcode, location },
            DescriptorViolationValue::Scalar { location, .. } => {
                DescriptorViolationKind::Scalar(location)
            }
            DescriptorViolationValue::IndirectCount { .. } => DescriptorViolationKind::Scalar(
                DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                    .at_byte(DescriptorSchema::WORD3_OFFSET),
            ),
            DescriptorViolationValue::IndirectAlignment { .. } => DescriptorViolationKind::Scalar(
                DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
                    .at_byte(DescriptorSchema::WORD1_OFFSET),
            ),
        }
    }

    /// Returns the raw value that violated the field constraint.
    pub fn observed(self) -> u64 {
        match self.0 {
            DescriptorViolationValue::InvalidVersion { observed }
            | DescriptorViolationValue::InvalidSize { observed }
            | DescriptorViolationValue::InvalidOpcode { observed }
            | DescriptorViolationValue::InvalidFlags { observed } => u64::from(observed),
            DescriptorViolationValue::Reserved { observed, .. }
            | DescriptorViolationValue::Scalar { observed, .. }
            | DescriptorViolationValue::IndirectCount { observed, .. } => observed,
            DescriptorViolationValue::IndirectAlignment { list } => list.raw(),
        }
    }

    /// Returns the required value, accepted mask, maximum, or alignment.
    pub fn expected(self) -> u64 {
        match self.0 {
            DescriptorViolationValue::InvalidVersion { .. } => u64::from(ABI_VERSION),
            DescriptorViolationValue::InvalidSize { .. } => u64::from(DESCRIPTOR_SIZE),
            DescriptorViolationValue::InvalidOpcode { .. } => u64::from(VECTOR_ADD_OPCODE),
            DescriptorViolationValue::InvalidFlags { .. } => u64::from(DESCRIPTOR_FLAGS_MASK),
            DescriptorViolationValue::Reserved { .. } => 0,
            DescriptorViolationValue::Scalar { .. } => u64::from(MAX_TRANSFER_BYTES),
            DescriptorViolationValue::IndirectCount { .. } => u64::from(MAX_INDIRECT_DESTINATIONS),
            DescriptorViolationValue::IndirectAlignment { .. } => INDIRECT_ENTRY_ALIGNMENT,
        }
    }

    /// Returns the descriptor byte offset for descriptor-local violations.
    pub const fn descriptor_offset(self) -> Option<u64> {
        match self.0 {
            DescriptorViolationValue::InvalidVersion { .. } => Some(location_offset(
                DescriptorHeaderSchema::VERSION_IDENTITY
                    .at_byte(DescriptorHeaderSchema::VERSION_OFFSET),
            )),
            DescriptorViolationValue::InvalidSize { .. } => Some(location_offset(
                DescriptorHeaderSchema::SIZE_IDENTITY.at_byte(DescriptorHeaderSchema::SIZE_OFFSET),
            )),
            DescriptorViolationValue::InvalidOpcode { .. } => Some(location_offset(
                DescriptorHeaderSchema::OPCODE_IDENTITY
                    .at_byte(DescriptorHeaderSchema::OPCODE_OFFSET),
            )),
            DescriptorViolationValue::InvalidFlags { .. } => Some(location_offset(
                DescriptorHeaderSchema::FLAGS_IDENTITY
                    .at_byte(DescriptorHeaderSchema::FLAGS_OFFSET),
            )),
            DescriptorViolationValue::Reserved { location, .. }
            | DescriptorViolationValue::Scalar { location, .. } => Some(location_offset(location)),
            DescriptorViolationValue::IndirectCount { .. }
            | DescriptorViolationValue::IndirectAlignment { .. } => None,
        }
    }

    /// Returns the list address for indirect-list violations.
    pub const fn indirect_list_address(self) -> Option<DmaAddress> {
        match self.0 {
            DescriptorViolationValue::IndirectCount { list, .. }
            | DescriptorViolationValue::IndirectAlignment { list } => Some(list),
            DescriptorViolationValue::InvalidVersion { .. }
            | DescriptorViolationValue::InvalidSize { .. }
            | DescriptorViolationValue::InvalidOpcode { .. }
            | DescriptorViolationValue::InvalidFlags { .. }
            | DescriptorViolationValue::Reserved { .. }
            | DescriptorViolationValue::Scalar { .. } => None,
        }
    }
}

impl From<DescriptorPayloadViolation> for DescriptorViolation {
    fn from(violation: DescriptorPayloadViolation) -> Self {
        match violation {
            DescriptorPayloadViolation::InvalidIndirectCount { list, observed } => {
                Self::indirect_count(DmaAddress::new(list), u64::from(observed))
            }
            DescriptorPayloadViolation::InvalidIndirectAlignment { address } => {
                Self::indirect_alignment(DmaAddress::new(address))
            }
        }
    }
}

impl std::error::Error for DescriptorViolation {}
