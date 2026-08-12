/// Global-register definitions without map placement.
pub mod global;
/// IRQ-register definitions without map placement.
pub mod irq;
/// Queue-register definitions without map placement.
pub mod queue;

use hardware_interface_schema::{register_field_schema, register_map};

pub use hardware_interface_schema::register::{Access as MmioAccess, ResetKind as MmioReset};

register_field_schema! { pub register_field U32Value { bit_width: 32 } }
register_field_schema! { pub register_field U64Value { bit_width: 64 } }

register_map! {
    pub map PdmaMmioMap {
        size: 0x1000,
        fixed: {
            g_magic: global::GMagic { id: 0, name: "G_MAGIC", offset: 0x000 },
            g_mmio_abi_version: global::GMmioAbiVersion { id: 1, name: "G_MMIO_ABI_VERSION", offset: 0x004 },
            g_req_abi_version: global::GReqAbiVersion { id: 2, name: "G_REQ_ABI_VERSION", offset: 0x008 },
            g_req_features: global::GReqFeatures { id: 3, name: "G_REQ_FEATURES", offset: 0x010 },
            g_reset_expected_epoch: global::GResetExpectedEpoch { id: 4, name: "G_RESET_EXPECTED_EPOCH", offset: 0x018 },
            g_command: global::GCommand { id: 5, name: "G_COMMAND", offset: 0x020 },
            g_result: global::GResult { id: 6, name: "G_RESULT", offset: 0x024 },
            g_detail0: global::GDetail0 { id: 7, name: "G_DETAIL0", offset: 0x028 },
            g_detail1: global::GDetail1 { id: 8, name: "G_DETAIL1", offset: 0x030 },
            g_detail2: global::GDetail2 { id: 9, name: "G_DETAIL2", offset: 0x038 },
            g_rsp_abi_version: global::GRspAbiVersion { id: 10, name: "G_RSP_ABI_VERSION", offset: 0x040 },
            g_rsp_run_state: global::GRspRunState { id: 11, name: "G_RSP_RUN_STATE", offset: 0x044 },
            g_rsp_features: global::GRspFeatures { id: 12, name: "G_RSP_FEATURES", offset: 0x048 },
            g_rsp_epoch: global::GRspEpoch { id: 13, name: "G_RSP_EPOCH", offset: 0x050 },
            g_max_ring_depth: global::GMaxRingDepth { id: 14, name: "G_MAX_RING_DEPTH", offset: 0x058 },
            g_max_indirect_destinations: global::GMaxIndirectDestinations { id: 15, name: "G_MAX_INDIRECT_DESTINATIONS", offset: 0x05c },
            g_max_transfer_bytes: global::GMaxTransferBytes { id: 16, name: "G_MAX_TRANSFER_BYTES", offset: 0x060 },
            g_max_strided_rows: global::GMaxStridedRows { id: 17, name: "G_MAX_STRIDED_ROWS", offset: 0x064 },
            g_max_stride: global::GMaxStride { id: 18, name: "G_MAX_STRIDE", offset: 0x068 },
            q_expected_epoch: queue::QExpectedEpoch { id: 19, name: "Q_EXPECTED_EPOCH", offset: 0x100 },
            q_id: queue::QId { id: 20, name: "Q_ID", offset: 0x108 },
            q_expected_cursor: queue::QExpectedCursor { id: 21, name: "Q_EXPECTED_CURSOR", offset: 0x110 },
            q_new_cursor: queue::QNewCursor { id: 22, name: "Q_NEW_CURSOR", offset: 0x118 },
            q_cfg_sq_depth: queue::QCfgSqDepth { id: 23, name: "Q_CFG_SQ_DEPTH", offset: 0x120 },
            q_cfg_cq_depth: queue::QCfgCqDepth { id: 24, name: "Q_CFG_CQ_DEPTH", offset: 0x124 },
            q_cfg_event_policy: queue::QCfgEventPolicy { id: 25, name: "Q_CFG_EVENT_POLICY", offset: 0x128 },
            q_cfg_software_owner: queue::QCfgSoftwareOwner { id: 26, name: "Q_CFG_SOFTWARE_OWNER", offset: 0x130 },
            q_sq_base: queue::QSqBase { id: 27, name: "Q_SQ_BASE", offset: 0x138 },
            q_sq_depth: queue::QSqDepth { id: 28, name: "Q_SQ_DEPTH", offset: 0x140 },
            q_cq_base: queue::QCqBase { id: 29, name: "Q_CQ_BASE", offset: 0x148 },
            q_cq_depth: queue::QCqDepth { id: 30, name: "Q_CQ_DEPTH", offset: 0x150 },
            q_eq_base: queue::QEqBase { id: 31, name: "Q_EQ_BASE", offset: 0x158 },
            q_eq_depth: queue::QEqDepth { id: 32, name: "Q_EQ_DEPTH", offset: 0x160 },
            q_command: queue::QCommand { id: 33, name: "Q_COMMAND", offset: 0x168 },
            q_result: queue::QResult { id: 34, name: "Q_RESULT", offset: 0x16c },
            q_detail0: queue::QDetail0 { id: 35, name: "Q_DETAIL0", offset: 0x170 },
            q_detail1: queue::QDetail1 { id: 36, name: "Q_DETAIL1", offset: 0x178 },
            q_detail2: queue::QDetail2 { id: 37, name: "Q_DETAIL2", offset: 0x180 },
            q_rsp_epoch: queue::QRspEpoch { id: 38, name: "Q_RSP_EPOCH", offset: 0x188 },
            q_rsp_id: queue::QRspId { id: 39, name: "Q_RSP_ID", offset: 0x190 },
            q_rsp_sq_producer: queue::QRspSqProducer { id: 40, name: "Q_RSP_SQ_PRODUCER", offset: 0x198 },
            q_rsp_sq_captured: queue::QRspSqCaptured { id: 41, name: "Q_RSP_SQ_CAPTURED", offset: 0x1a0 },
            q_rsp_sq_consumer: queue::QRspSqConsumer { id: 42, name: "Q_RSP_SQ_CONSUMER", offset: 0x1a8 },
            q_rsp_cq_producer: queue::QRspCqProducer { id: 43, name: "Q_RSP_CQ_PRODUCER", offset: 0x1b0 },
            q_rsp_cq_consumer: queue::QRspCqConsumer { id: 44, name: "Q_RSP_CQ_CONSUMER", offset: 0x1b8 },
            q_rsp_eq_producer: queue::QRspEqProducer { id: 45, name: "Q_RSP_EQ_PRODUCER", offset: 0x1c0 },
            q_rsp_eq_consumer: queue::QRspEqConsumer { id: 46, name: "Q_RSP_EQ_CONSUMER", offset: 0x1c8 },
            i_expected_epoch: irq::IExpectedEpoch { id: 47, name: "I_EXPECTED_EPOCH", offset: 0x200 },
            i_source_id: irq::ISourceId { id: 48, name: "I_SOURCE_ID", offset: 0x208 },
            i_source_kind: irq::ISourceKind { id: 49, name: "I_SOURCE_KIND", offset: 0x210 },
            i_route_kind: irq::IRouteKind { id: 50, name: "I_ROUTE_KIND", offset: 0x214 },
            i_route_vector: irq::IRouteVector { id: 51, name: "I_ROUTE_VECTOR", offset: 0x218 },
            i_command: irq::ICommand { id: 52, name: "I_COMMAND", offset: 0x220 },
            i_result: irq::IResult { id: 53, name: "I_RESULT", offset: 0x224 },
            i_detail0: irq::IDetail0 { id: 54, name: "I_DETAIL0", offset: 0x228 },
            i_detail1: irq::IDetail1 { id: 55, name: "I_DETAIL1", offset: 0x230 },
            i_detail2: irq::IDetail2 { id: 56, name: "I_DETAIL2", offset: 0x238 },
            i_rsp_epoch: irq::IRspEpoch { id: 57, name: "I_RSP_EPOCH", offset: 0x240 },
            i_rsp_source_id: irq::IRspSourceId { id: 58, name: "I_RSP_SOURCE_ID", offset: 0x248 },
            i_rsp_source_kind: irq::IRspSourceKind { id: 59, name: "I_RSP_SOURCE_KIND", offset: 0x250 },
            i_rsp_state: irq::IRspState { id: 60, name: "I_RSP_STATE", offset: 0x254 },
            i_rsp_route_kind: irq::IRspRouteKind { id: 61, name: "I_RSP_ROUTE_KIND", offset: 0x258 },
            i_rsp_notification_kind: irq::IRspNotificationKind { id: 62, name: "I_RSP_NOTIFICATION_KIND", offset: 0x25c },
            i_rsp_vector: irq::IRspVector { id: 63, name: "I_RSP_VECTOR", offset: 0x260 },
            i_rsp_message_id: irq::IRspMessageId { id: 64, name: "I_RSP_MESSAGE_ID", offset: 0x268 },
            i_rsp_pending_messages: irq::IRspPendingMessages { id: 65, name: "I_RSP_PENDING_MESSAGES", offset: 0x270 },
        },
        banks: {}
    }
}

/// Lossless domain for raw MMIO transaction byte counts.
pub type MmioByteCount = u128;

/// Exact byte size derived from the generated MMIO map.
pub const MMIO_REGION_SIZE: MmioByteCount = PdmaMmioMap::SPEC.size() as MmioByteCount;

/// Stable identity for every registered MMIO offset.
pub type RegisterId = PdmaMmioMapRegisterId;

/// Canonical generated placement metadata for one PDMA register.
pub type RegisterSpec = hardware_interface_schema::register::RegisterSpec<RegisterId>;
