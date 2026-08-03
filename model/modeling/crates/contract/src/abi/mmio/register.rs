mod global;
mod irq;
mod queue;

/// Lossless domain for raw MMIO transaction byte counts.
pub type MmioByteCount = u128;
/// Exact byte size of the MMIO region.
pub const MMIO_REGION_SIZE: MmioByteCount = 0x1000;

/// Host access permission for a register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioAccess {
    /// Host reads only.
    ReadOnly,
    /// Host reads and writes.
    ReadWrite,
    /// Host writes only.
    WriteOnly,
}

impl MmioAccess {
    /// Returns the stable numeric access encoding.
    pub const fn code(self) -> u32 {
        match self {
            Self::ReadOnly => 0,
            Self::ReadWrite => 1,
            Self::WriteOnly => 2,
        }
    }
}

/// Register reset classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioReset {
    /// A fixed value unaffected by whole-device reset.
    Constant,
    /// Zero at construction and after whole-device reset.
    Zero,
    /// Zero at construction and populated by successful reset.
    PostReset,
}

impl MmioReset {
    /// Returns the stable numeric reset-class encoding.
    pub const fn code(self) -> u32 {
        match self {
            Self::Constant => 0,
            Self::Zero => 1,
            Self::PostReset => 2,
        }
    }
}

macro_rules! register_ids {
    ($($id:ident),+ $(,)?) => {
        /// Stable identity for every registered MMIO offset.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum RegisterId { $(#[doc = "MMIO register identity."] $id),+ }
    };
}

register_ids!(
    GMagic,
    GMmioAbiVersion,
    GReqAbiVersion,
    GReqFeatures,
    GResetExpectedEpoch,
    GCommand,
    GResult,
    GDetail0,
    GDetail1,
    GDetail2,
    GRspAbiVersion,
    GRspRunState,
    GRspFeatures,
    GRspEpoch,
    GMaxRingDepth,
    GMaxIndirectDestinations,
    GMaxTransferBytes,
    GMaxStridedRows,
    GMaxStride,
    QExpectedEpoch,
    QId,
    QExpectedCursor,
    QNewCursor,
    QCfgSqDepth,
    QCfgCqDepth,
    QCfgEventPolicy,
    QCfgSoftwareOwner,
    QSqBase,
    QSqDepth,
    QCqBase,
    QCqDepth,
    QEqBase,
    QEqDepth,
    QCommand,
    QResult,
    QDetail0,
    QDetail1,
    QDetail2,
    QRspEpoch,
    QRspId,
    QRspSqProducer,
    QRspSqCaptured,
    QRspSqConsumer,
    QRspCqProducer,
    QRspCqConsumer,
    QRspEqProducer,
    QRspEqConsumer,
    IExpectedEpoch,
    ISourceId,
    ISourceKind,
    IRouteKind,
    IRouteVector,
    ICommand,
    IResult,
    IDetail0,
    IDetail1,
    IDetail2,
    IRspEpoch,
    IRspSourceId,
    IRspSourceKind,
    IRspState,
    IRspRouteKind,
    IRspNotificationKind,
    IRspVector,
    IRspMessageId,
    IRspPendingMessages,
);

/// One canonical numeric register description.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterSpec {
    id: RegisterId,
    name: &'static str,
    offset: u32,
    width: MmioByteCount,
    access: MmioAccess,
    reset: MmioReset,
    reset_value: u64,
    writable_mask: u64,
}

impl RegisterSpec {
    pub(super) const fn new(id: RegisterId, name: &'static str, values: RegisterValues) -> Self {
        Self {
            id,
            name,
            offset: values.offset,
            width: values.width,
            access: values.access,
            reset: values.reset,
            reset_value: values.reset_value,
            writable_mask: values.writable_mask,
        }
    }
    /// Returns the stable register identity.
    pub const fn id(self) -> RegisterId {
        self.id
    }
    /// Returns the canonical uppercase register name.
    pub const fn name(self) -> &'static str {
        self.name
    }
    /// Returns the byte offset within the MMIO region.
    pub const fn offset(self) -> u32 {
        self.offset
    }
    /// Returns the exact transaction width in bytes.
    pub const fn width(self) -> MmioByteCount {
        self.width
    }
    /// Returns the host access permission.
    pub const fn access(self) -> MmioAccess {
        self.access
    }
    /// Returns the reset classification.
    pub const fn reset(self) -> MmioReset {
        self.reset
    }
    /// Returns the construction reset value.
    pub const fn reset_value(self) -> u64 {
        self.reset_value
    }
    /// Returns the host-writable bit mask.
    pub const fn writable_mask(self) -> u64 {
        self.writable_mask
    }
}

#[derive(Clone, Copy)]
pub(super) struct RegisterValues {
    pub(super) offset: u32,
    pub(super) width: MmioByteCount,
    pub(super) access: MmioAccess,
    pub(super) reset: MmioReset,
    pub(super) reset_value: u64,
    pub(super) writable_mask: u64,
}

/// Read-only view of the canonical split register tables.
#[derive(Clone, Copy, Debug)]
pub struct RegisterRegistry;

impl RegisterRegistry {
    /// Returns the exact register count.
    pub const fn len(self) -> usize {
        66
    }
    /// Reports whether the registry is empty.
    pub const fn is_empty(self) -> bool {
        false
    }
    /// Iterates registers in ascending offset order.
    pub fn iter(self) -> impl Iterator<Item = &'static RegisterSpec> {
        global::REGISTERS
            .iter()
            .chain(queue::REGISTERS.iter())
            .chain(irq::REGISTERS.iter())
    }
}

/// Canonical 66-register MMIO registry.
pub const MMIO_REGISTERS: RegisterRegistry = RegisterRegistry;
