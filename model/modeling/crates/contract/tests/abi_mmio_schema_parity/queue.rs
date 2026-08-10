use super::ExpectedRegister;
use pdma_model_contract::abi::mmio::{MmioAccess, MmioReset, RegisterId};

macro_rules! r {
    ($id:ident,$name:literal,$offset:literal,$width:literal,$access:ident,$reset:ident,$value:expr,$mask:expr) => {
        ExpectedRegister {
            id: RegisterId::$id,
            name: $name,
            offset: $offset,
            width: $width,
            access: MmioAccess::$access,
            reset: MmioReset::$reset,
            reset_value: $value,
            writable_mask: $mask,
        }
    };
}

pub(super) const REGISTERS: [ExpectedRegister; 28] = [
    r!(
        QExpectedEpoch,
        "Q_EXPECTED_EPOCH",
        0x100,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(QId, "Q_ID", 0x108, 8, ReadWrite, Zero, 0, u64::MAX),
    r!(
        QExpectedCursor,
        "Q_EXPECTED_CURSOR",
        0x110,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        QNewCursor,
        "Q_NEW_CURSOR",
        0x118,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        QCfgSqDepth,
        "Q_CFG_SQ_DEPTH",
        0x120,
        4,
        ReadWrite,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(
        QCfgCqDepth,
        "Q_CFG_CQ_DEPTH",
        0x124,
        4,
        ReadWrite,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(
        QCfgEventPolicy,
        "Q_CFG_EVENT_POLICY",
        0x128,
        4,
        ReadWrite,
        Zero,
        0,
        0
    ),
    r!(
        QCfgSoftwareOwner,
        "Q_CFG_SOFTWARE_OWNER",
        0x130,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(QSqBase, "Q_SQ_BASE", 0x138, 8, ReadWrite, Zero, 0, u64::MAX),
    r!(
        QSqDepth,
        "Q_SQ_DEPTH",
        0x140,
        4,
        ReadWrite,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(QCqBase, "Q_CQ_BASE", 0x148, 8, ReadWrite, Zero, 0, u64::MAX),
    r!(
        QCqDepth,
        "Q_CQ_DEPTH",
        0x150,
        4,
        ReadWrite,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(QEqBase, "Q_EQ_BASE", 0x158, 8, ReadWrite, Zero, 0, u64::MAX),
    r!(
        QEqDepth,
        "Q_EQ_DEPTH",
        0x160,
        4,
        ReadWrite,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(
        QCommand,
        "Q_COMMAND",
        0x168,
        4,
        WriteOnly,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(QResult, "Q_RESULT", 0x16c, 4, ReadOnly, Zero, 0, 0),
    r!(QDetail0, "Q_DETAIL0", 0x170, 8, ReadOnly, Zero, 0, 0),
    r!(QDetail1, "Q_DETAIL1", 0x178, 8, ReadOnly, Zero, 0, 0),
    r!(QDetail2, "Q_DETAIL2", 0x180, 8, ReadOnly, Zero, 0, 0),
    r!(QRspEpoch, "Q_RSP_EPOCH", 0x188, 8, ReadOnly, Zero, 0, 0),
    r!(QRspId, "Q_RSP_ID", 0x190, 8, ReadOnly, Zero, 0, 0),
    r!(
        QRspSqProducer,
        "Q_RSP_SQ_PRODUCER",
        0x198,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        QRspSqCaptured,
        "Q_RSP_SQ_CAPTURED",
        0x1a0,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        QRspSqConsumer,
        "Q_RSP_SQ_CONSUMER",
        0x1a8,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        QRspCqProducer,
        "Q_RSP_CQ_PRODUCER",
        0x1b0,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        QRspCqConsumer,
        "Q_RSP_CQ_CONSUMER",
        0x1b8,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        QRspEqProducer,
        "Q_RSP_EQ_PRODUCER",
        0x1c0,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        QRspEqConsumer,
        "Q_RSP_EQ_CONSUMER",
        0x1c8,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
];
