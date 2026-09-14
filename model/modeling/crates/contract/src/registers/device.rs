use super::{ChannelBlock, DeviceControl};
use hardware_interface_schema::{
    Documentation, RegisterCollection, RegisterLayout, RegisterLayoutGeometry, RegisterMapError,
    RegisterMapPlacement, ResolvedRegisterMap, checked_placement_end, finish_ordered_register_map,
    inferred_align_up, register_schema,
};

register_schema! {
    #[short_description = "Hardware channel count"]
    #[description = "Read-only count supplied by this hardware instance. Hardware initializes the value after construction and preserves it across device reset."]
    pub register ChannelCount {
        width: 4, endian: Little, access: ReadOnly,
        reset: PostReset(0), writable_mask: 0, fields: {},
    }
}

/// Top-level device register placement identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRegistersId {
    /// Device-wide discovery and recovery registers.
    Control,
    /// Independent channel register blocks.
    Channels,
}

/// Invalid instance geometry or generated child schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceLayoutError {
    /// The channel count is zero or exceeds the discovery register width.
    InvalidChannelCount {
        /// Requested hardware channel count.
        count: usize,
    },
    /// The repeated register extent cannot fit in the address space.
    AddressOverflow,
    /// A generated child schema could not be resolved.
    Schema(RegisterMapError<DeviceRegistersId>),
}

impl std::fmt::Display for DeviceLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PDMA device layout: {self:?}")
    }
}

impl std::error::Error for DeviceLayoutError {}

/// Instance-count composition of the generated device and channel blocks.
#[derive(Debug)]
pub struct DeviceRegisters;

impl DeviceRegisters {
    /// Root register alignment.
    pub const ALIGNMENT: usize = ChannelBlock::ALIGNMENT;
    /// Register-map meaning shared by every device instance.
    pub const DOCUMENTATION: Documentation = Documentation {
        short_description: Some("PDMA device registers"),
        description: Some(
            "Device-wide discovery and recovery controls followed by the instance's independent channel blocks. Register transactions use little-endian bytes and must match an exact register start and width; layout gaps are not registers.",
        ),
    };
    /// Fixed device-wide control placement.
    pub const CONTROL: RegisterMapPlacement<DeviceRegistersId> = RegisterMapPlacement::new(
        DeviceRegistersId::Control,
        "control",
        0,
        1,
        0,
        RegisterLayoutGeometry::new(DeviceControl::ALIGNMENT, DeviceControl::EXTENT),
    )
    .with_documentation(Documentation {
        short_description: Some("Device management interface"),
        description: Some(
            "Hardware channel discovery, device fatal notification and whole-device reset.",
        ),
    });
    /// Byte offset of the first channel block.
    pub const CHANNELS_OFFSET: usize =
        inferred_align_up(DeviceControl::EXTENT, ChannelBlock::ALIGNMENT);
    /// Byte distance between adjacent channel blocks.
    pub const CHANNEL_STRIDE: usize =
        inferred_align_up(ChannelBlock::EXTENT, ChannelBlock::ALIGNMENT);

    /// Returns the exclusive register extent for one hardware instance.
    ///
    /// # Errors
    /// Rejects zero, a count wider than discovery, or overflowing address geometry.
    pub fn extent(channel_count: usize) -> Result<usize, DeviceLayoutError> {
        if channel_count == 0 || u32::try_from(channel_count).is_err() {
            return Err(DeviceLayoutError::InvalidChannelCount {
                count: channel_count,
            });
        }
        checked_placement_end(
            Self::CHANNELS_OFFSET,
            channel_count,
            Self::CHANNEL_STRIDE,
            ChannelBlock::EXTENT,
        )
        .ok_or(DeviceLayoutError::AddressOverflow)
    }

    /// Returns repeated placement metadata for the requested hardware instance.
    ///
    /// # Errors
    /// Rejects an invalid count or overflowing address geometry.
    pub fn channels(
        channel_count: usize,
    ) -> Result<RegisterMapPlacement<DeviceRegistersId>, DeviceLayoutError> {
        Self::extent(channel_count)?;
        Ok(RegisterMapPlacement::new(
            DeviceRegistersId::Channels, "channels", Self::CHANNELS_OFFSET, channel_count, Self::CHANNEL_STRIDE,
            RegisterLayoutGeometry::new(ChannelBlock::ALIGNMENT, ChannelBlock::EXTENT),
        ).with_documentation(Documentation {
            short_description: Some("Independent channel interfaces"),
            description: Some("Repeated command/response rings and channel management. Each block owns its configuration, cursors, faults, notifications and recovery."),
        }))
    }

    /// Expands generated child blocks for one hardware instance.
    ///
    /// # Errors
    /// Rejects invalid geometry or an invalid generated child schema.
    pub fn resolve(
        channel_count: usize,
    ) -> Result<ResolvedRegisterMap<DeviceRegistersId>, DeviceLayoutError> {
        let channels = Self::channels(channel_count)?;
        let extent = Self::extent(channel_count)?;
        let mut registers = Vec::new();
        let mut placements = Vec::new();
        let mut declaration_index = 0;
        for placement in [Self::CONTROL, channels] {
            for index in 0..placement.count() {
                let (path, repeated) = match placement.id() {
                    DeviceRegistersId::Control => (String::from("DeviceRegisters.control"), None),
                    DeviceRegistersId::Channels => (
                        format!("DeviceRegisters.channels[{index}]"),
                        Some(("channels", index)),
                    ),
                };
                let collection = RegisterCollection {
                    documentation: placement.documentation(),
                    placements: &mut placements,
                    id: placement.id(),
                    name: placement.name(),
                    path,
                    offset: placement.offset() + index * placement.stride(),
                    declaration_index: &mut declaration_index,
                    repeated,
                    output: &mut registers,
                };
                match placement.id() {
                    DeviceRegistersId::Control => DeviceControl::append_registers(collection),
                    DeviceRegistersId::Channels => ChannelBlock::append_registers(collection),
                }
            }
        }
        finish_ordered_register_map("DeviceRegisters", extent, registers)
            .map(|map| {
                map.with_placements(placements)
                    .with_documentation(Self::DOCUMENTATION)
            })
            .map_err(DeviceLayoutError::Schema)
    }
}

/// A channel selection, checked against hardware discovery by its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(usize);

impl ChannelId {
    /// Creates a channel selection without assuming a device's channel count.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based channel index.
    pub const fn index(self) -> usize {
        self.0
    }

    /// Returns the channel block address, or `None` on address arithmetic overflow.
    pub const fn register_offset(self) -> Option<usize> {
        match self.0.checked_mul(DeviceRegisters::CHANNEL_STRIDE) {
            Some(offset) => offset.checked_add(DeviceRegisters::CHANNELS_OFFSET),
            None => None,
        }
    }
}
