use hardware_interface_schema::RegisterLocation as GeneratedLocation;
use hardware_interface_schema::register::RegisterSpec;

use super::{
    GlobalRegisters, GlobalRegistersRegisterId, IrqSourceRegisters, IrqSourceRegistersRegisterId,
    PdmaRegisterMap, QueueInstanceRegisters, QueueInstanceRegistersRegisterId, QueueRegisters,
    QueueRegistersRegisterId,
};

/// Invalid dynamic register-address geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterAddressError {
    /// The computed register start exceeds the numeric u32 aperture.
    AddressOverflow,
    /// Only source slots zero (completion) and one (event) exist.
    SourceIndex {
        /// Requested source slot.
        index: u32,
    },
    /// The local byte offset is not an exact register start in the selected block.
    LocalOffset {
        /// Requested block-relative offset.
        offset: usize,
    },
}

impl core::fmt::Display for RegisterAddressError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "PDMA register address failed: {self:?}")
    }
}

impl std::error::Error for RegisterAddressError {}

/// Exact register identity with block-local generated permission metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterLocation {
    /// One device-wide register.
    Global {
        /// Generated register with its global-block offset.
        register: RegisterSpec<GlobalRegistersRegisterId>,
    },
    /// One queue's configuration or cursor register.
    Queue {
        /// Zero-based queue instance.
        index: u32,
        /// Generated register with its queue-block offset.
        register: RegisterSpec<QueueRegistersRegisterId>,
    },
    /// One queue's completion or event source register.
    Irq {
        /// Zero-based queue instance.
        index: u32,
        /// Zero for completion, one for event.
        source_index: u32,
        /// Generated register with its source-block offset.
        register: RegisterSpec<IrqSourceRegistersRegisterId>,
    },
}

/// Computes a queue register's numeric offset without a static queue-count cap.
///
/// # Errors
/// Rejects a non-register local offset or arithmetic outside the u32 aperture.
pub fn queue_offset(index: u32, local: usize) -> Result<u32, RegisterAddressError> {
    QueueRegisters::locate(local).ok_or(RegisterAddressError::LocalOffset { offset: local })?;
    let relative = QueueInstanceRegisters::QUEUE
        .offset()
        .checked_add(local)
        .ok_or(RegisterAddressError::AddressOverflow)?;
    instance_offset(index, relative)
}

/// Computes one queue source's numeric offset from the generated source stride.
///
/// # Errors
/// Rejects an absent source slot, non-register local offset, or address overflow.
pub fn irq_offset(
    index: u32,
    source_index: u32,
    local: usize,
) -> Result<u32, RegisterAddressError> {
    let source = usize::try_from(source_index).map_err(|_| RegisterAddressError::SourceIndex {
        index: source_index,
    })?;
    if source >= QueueInstanceRegisters::SOURCES.count() {
        return Err(RegisterAddressError::SourceIndex {
            index: source_index,
        });
    }
    IrqSourceRegisters::locate(local).ok_or(RegisterAddressError::LocalOffset { offset: local })?;
    let relative = source
        .checked_mul(QueueInstanceRegisters::SOURCES.stride())
        .and_then(|offset| offset.checked_add(QueueInstanceRegisters::SOURCES.offset()))
        .and_then(|offset| offset.checked_add(local))
        .ok_or(RegisterAddressError::AddressOverflow)?;
    instance_offset(index, relative)
}

fn instance_offset(index: u32, relative: usize) -> Result<u32, RegisterAddressError> {
    let index = usize::try_from(index).map_err(|_| RegisterAddressError::AddressOverflow)?;
    let offset = index
        .checked_mul(PdmaRegisterMap::QUEUES.stride())
        .and_then(|offset| offset.checked_add(PdmaRegisterMap::QUEUES.offset()))
        .and_then(|offset| offset.checked_add(relative))
        .ok_or(RegisterAddressError::AddressOverflow)?;
    u32::try_from(offset).map_err(|_| RegisterAddressError::AddressOverflow)
}

/// Locates an exact register start; instance existence remains device policy.
pub fn locate(offset: u32) -> Option<RegisterLocation> {
    let offset = usize::try_from(offset).ok()?;
    if offset < PdmaRegisterMap::QUEUES.offset() {
        let local = offset.checked_sub(PdmaRegisterMap::GLOBAL.offset())?;
        return Some(RegisterLocation::Global {
            register: register_spec(GlobalRegisters::locate(local)?),
        });
    }
    let relative = offset.checked_sub(PdmaRegisterMap::QUEUES.offset())?;
    let index = u32::try_from(relative / PdmaRegisterMap::QUEUES.stride()).ok()?;
    let local = relative % PdmaRegisterMap::QUEUES.stride();
    let register = register_spec(QueueInstanceRegisters::locate(local)?);
    match register.id() {
        QueueInstanceRegistersRegisterId::Queue => Some(RegisterLocation::Queue {
            index,
            register: register_spec(QueueRegisters::locate(
                local.checked_sub(QueueInstanceRegisters::QUEUE.offset())?,
            )?),
        }),
        QueueInstanceRegistersRegisterId::Sources => {
            let relative = local.checked_sub(QueueInstanceRegisters::SOURCES.offset())?;
            let stride = QueueInstanceRegisters::SOURCES.stride();
            Some(RegisterLocation::Irq {
                index,
                source_index: u32::try_from(relative / stride).ok()?,
                register: register_spec(IrqSourceRegisters::locate(relative % stride)?),
            })
        }
    }
}

const fn register_spec<I: Copy>(location: GeneratedLocation<I>) -> RegisterSpec<I> {
    match location {
        GeneratedLocation::Fixed { register, .. }
        | GeneratedLocation::Repeated { register, .. } => register,
    }
}
