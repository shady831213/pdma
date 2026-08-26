/// Global-register definitions without map placement.
pub mod global;
/// IRQ-register definitions without map placement.
pub mod irq;
/// Queue-register definitions without map placement.
pub mod queue;

use hardware_interface_schema::register_map;

pub use hardware_interface_schema::register::{Access as MmioAccess, ResetKind as MmioReset};

register_map! {
    pub map PdmaMmioMap {
        alignment: 8,
        extent: 0x1000,
        placements: [
            g_magic: global::GMagic { offset: 0x000 },
            g_mmio_abi_version: global::GMmioAbiVersion { offset: 0x004 },
            g_req_abi_version: global::GReqAbiVersion { offset: 0x008 },
            g_req_features: global::GReqFeatures { offset: 0x010 },
            g_reset_expected_epoch: global::GResetExpectedEpoch { offset: 0x018 },
            g_command: global::GCommand { offset: 0x020 },
            g_result: global::GResult { offset: 0x024 },
            g_detail0: global::GDetail0 { offset: 0x028 },
            g_detail1: global::GDetail1 { offset: 0x030 },
            g_detail2: global::GDetail2 { offset: 0x038 },
            g_rsp_abi_version: global::GRspAbiVersion { offset: 0x040 },
            g_rsp_run_state: global::GRspRunState { offset: 0x044 },
            g_rsp_features: global::GRspFeatures { offset: 0x048 },
            g_rsp_epoch: global::GRspEpoch { offset: 0x050 },
            g_max_ring_depth: global::GMaxRingDepth { offset: 0x058 },
            g_max_indirect_destinations: global::GMaxIndirectDestinations { offset: 0x05c },
            g_max_transfer_bytes: global::GMaxTransferBytes { offset: 0x060 },
            g_max_strided_rows: global::GMaxStridedRows { offset: 0x064 },
            g_max_stride: global::GMaxStride { offset: 0x068 },
            q_expected_epoch: queue::QExpectedEpoch { offset: 0x100 },
            q_id: queue::QId { offset: 0x108 },
            q_expected_cursor: queue::QExpectedCursor { offset: 0x110 },
            q_new_cursor: queue::QNewCursor { offset: 0x118 },
            q_cfg_sq_depth: queue::QCfgSqDepth { offset: 0x120 },
            q_cfg_cq_depth: queue::QCfgCqDepth { offset: 0x124 },
            q_cfg_event_policy: queue::QCfgEventPolicy { offset: 0x128 },
            q_cfg_software_owner: queue::QCfgSoftwareOwner { offset: 0x130 },
            q_sq_base: queue::QSqBase { offset: 0x138 },
            q_sq_depth: queue::QSqDepth { offset: 0x140 },
            q_cq_base: queue::QCqBase { offset: 0x148 },
            q_cq_depth: queue::QCqDepth { offset: 0x150 },
            q_eq_base: queue::QEqBase { offset: 0x158 },
            q_eq_depth: queue::QEqDepth { offset: 0x160 },
            q_command: queue::QCommand { offset: 0x168 },
            q_result: queue::QResult { offset: 0x16c },
            q_detail0: queue::QDetail0 { offset: 0x170 },
            q_detail1: queue::QDetail1 { offset: 0x178 },
            q_detail2: queue::QDetail2 { offset: 0x180 },
            q_rsp_epoch: queue::QRspEpoch { offset: 0x188 },
            q_rsp_id: queue::QRspId { offset: 0x190 },
            q_rsp_sq_producer: queue::QRspSqProducer { offset: 0x198 },
            q_rsp_sq_captured: queue::QRspSqCaptured { offset: 0x1a0 },
            q_rsp_sq_consumer: queue::QRspSqConsumer { offset: 0x1a8 },
            q_rsp_cq_producer: queue::QRspCqProducer { offset: 0x1b0 },
            q_rsp_cq_consumer: queue::QRspCqConsumer { offset: 0x1b8 },
            q_rsp_eq_producer: queue::QRspEqProducer { offset: 0x1c0 },
            q_rsp_eq_consumer: queue::QRspEqConsumer { offset: 0x1c8 },
            i_expected_epoch: irq::IExpectedEpoch { offset: 0x200 },
            i_source_id: irq::ISourceId { offset: 0x208 },
            i_source_kind: irq::ISourceKind { offset: 0x210 },
            i_route_kind: irq::IRouteKind { offset: 0x214 },
            i_route_vector: irq::IRouteVector { offset: 0x218 },
            i_command: irq::ICommand { offset: 0x220 },
            i_result: irq::IResult { offset: 0x224 },
            i_detail0: irq::IDetail0 { offset: 0x228 },
            i_detail1: irq::IDetail1 { offset: 0x230 },
            i_detail2: irq::IDetail2 { offset: 0x238 },
            i_rsp_epoch: irq::IRspEpoch { offset: 0x240 },
            i_rsp_source_id: irq::IRspSourceId { offset: 0x248 },
            i_rsp_source_kind: irq::IRspSourceKind { offset: 0x250 },
            i_rsp_state: irq::IRspState { offset: 0x254 },
            i_rsp_route_kind: irq::IRspRouteKind { offset: 0x258 },
            i_rsp_notification_kind: irq::IRspNotificationKind { offset: 0x25c },
            i_rsp_vector: irq::IRspVector { offset: 0x260 },
            i_rsp_message_id: irq::IRspMessageId { offset: 0x268 },
            i_rsp_pending_messages: irq::IRspPendingMessages { offset: 0x270 },
        ],
    }
}

/// Lossless domain for raw MMIO transaction byte counts.
pub type MmioByteCount = u128;

/// Exact byte size derived from the generated MMIO map.
pub const MMIO_REGION_SIZE: MmioByteCount = PdmaMmioMap::EXTENT as MmioByteCount;

/// Stable identity for every registered MMIO offset.
pub type RegisterId = PdmaMmioMapRegisterId;

/// Canonical generated placement metadata for one PDMA register.
pub type RegisterSpec = hardware_interface_schema::register::RegisterSpec<RegisterId>;
