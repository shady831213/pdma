mod global;
mod irq;
mod queue;

use hardware_interface_schema::{
    register::{Access, RegisterSpec as SchemaRegisterSpec, ResetKind},
    register_map,
};

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

register_map! {
    pub map PdmaMmioMap {
        size: 0x1000,
        fixed: {},
        fixed_groups: {
            global::GlobalRegisters: [g_magic, g_mmio_abi_version, g_req_abi_version,
                g_req_features, g_reset_expected_epoch, g_command, g_result, g_detail0,
                g_detail1, g_detail2, g_rsp_abi_version, g_rsp_run_state, g_rsp_features,
                g_rsp_epoch, g_max_ring_depth, g_max_indirect_destinations,
                g_max_transfer_bytes, g_max_strided_rows, g_max_stride],
            queue::QueueRegisters: [q_expected_epoch, q_id, q_expected_cursor, q_new_cursor,
                q_cfg_sq_depth, q_cfg_cq_depth, q_cfg_event_policy, q_cfg_software_owner,
                q_sq_base, q_sq_depth, q_cq_base, q_cq_depth, q_eq_base, q_eq_depth,
                q_command, q_result, q_detail0, q_detail1, q_detail2, q_rsp_epoch, q_rsp_id,
                q_rsp_sq_producer, q_rsp_sq_captured, q_rsp_sq_consumer, q_rsp_cq_producer,
                q_rsp_cq_consumer, q_rsp_eq_producer, q_rsp_eq_consumer],
            irq::IrqRegisters: [i_expected_epoch, i_source_id, i_source_kind, i_route_kind,
                i_route_vector, i_command, i_result, i_detail0, i_detail1, i_detail2,
                i_rsp_epoch, i_rsp_source_id, i_rsp_source_kind, i_rsp_state, i_rsp_route_kind,
                i_rsp_notification_kind, i_rsp_vector, i_rsp_message_id,
                i_rsp_pending_messages],
        },
        banks: {}
    }
}

/// Stable identity for every registered MMIO offset.
pub type RegisterId = PdmaMmioMapRegisterId;

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
    #[allow(
        clippy::cast_possible_truncation,
        reason = "register_schema rejects offsets that do not fit u32"
    )]
    pub(super) const fn from_schema(schema: SchemaRegisterSpec<RegisterId>) -> Self {
        Self {
            id: schema.id(),
            name: schema.name(),
            offset: schema.offset() as u32,
            width: schema.width() as MmioByteCount,
            access: match schema.access() {
                Access::ReadOnly => MmioAccess::ReadOnly,
                Access::ReadWrite => MmioAccess::ReadWrite,
                Access::WriteOnly => MmioAccess::WriteOnly,
            },
            reset: match schema.reset_kind() {
                ResetKind::Constant => MmioReset::Constant,
                ResetKind::Zero => MmioReset::Zero,
                ResetKind::PostReset => MmioReset::PostReset,
            },
            reset_value: schema.reset_value(),
            writable_mask: schema.writable_mask(),
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
