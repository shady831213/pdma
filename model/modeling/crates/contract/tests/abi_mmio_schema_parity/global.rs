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

pub(super) const REGISTERS: [ExpectedRegister; 19] = [
    r!(
        GMagic,
        "G_MAGIC",
        0x000,
        4,
        ReadOnly,
        Constant,
        0x414d_4450,
        0
    ),
    r!(
        GMmioAbiVersion,
        "G_MMIO_ABI_VERSION",
        0x004,
        4,
        ReadOnly,
        Constant,
        1,
        0
    ),
    r!(
        GReqAbiVersion,
        "G_REQ_ABI_VERSION",
        0x008,
        4,
        ReadWrite,
        Zero,
        0,
        0xffff
    ),
    r!(
        GReqFeatures,
        "G_REQ_FEATURES",
        0x010,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        GResetExpectedEpoch,
        "G_RESET_EXPECTED_EPOCH",
        0x018,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        GCommand,
        "G_COMMAND",
        0x020,
        4,
        WriteOnly,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(GResult, "G_RESULT", 0x024, 4, ReadOnly, Zero, 0, 0),
    r!(GDetail0, "G_DETAIL0", 0x028, 8, ReadOnly, Zero, 0, 0),
    r!(GDetail1, "G_DETAIL1", 0x030, 8, ReadOnly, Zero, 0, 0),
    r!(GDetail2, "G_DETAIL2", 0x038, 8, ReadOnly, Zero, 0, 0),
    r!(
        GRspAbiVersion,
        "G_RSP_ABI_VERSION",
        0x040,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        GRspRunState,
        "G_RSP_RUN_STATE",
        0x044,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        GRspFeatures,
        "G_RSP_FEATURES",
        0x048,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        GRspEpoch,
        "G_RSP_EPOCH",
        0x050,
        8,
        ReadOnly,
        PostReset,
        0,
        0
    ),
    r!(
        GMaxRingDepth,
        "G_MAX_RING_DEPTH",
        0x058,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        GMaxIndirectDestinations,
        "G_MAX_INDIRECT_DESTINATIONS",
        0x05c,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        GMaxTransferBytes,
        "G_MAX_TRANSFER_BYTES",
        0x060,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        GMaxStridedRows,
        "G_MAX_STRIDED_ROWS",
        0x064,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(GMaxStride, "G_MAX_STRIDE", 0x068, 4, ReadOnly, Zero, 0, 0),
];
