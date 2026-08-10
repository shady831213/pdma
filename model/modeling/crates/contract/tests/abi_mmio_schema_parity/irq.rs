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
        IExpectedEpoch,
        "I_EXPECTED_EPOCH",
        0x200,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        ISourceId,
        "I_SOURCE_ID",
        0x208,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        ISourceKind,
        "I_SOURCE_KIND",
        0x210,
        4,
        ReadWrite,
        Zero,
        0,
        1
    ),
    r!(IRouteKind, "I_ROUTE_KIND", 0x214, 4, ReadWrite, Zero, 0, 1),
    r!(
        IRouteVector,
        "I_ROUTE_VECTOR",
        0x218,
        8,
        ReadWrite,
        Zero,
        0,
        u64::MAX
    ),
    r!(
        ICommand,
        "I_COMMAND",
        0x220,
        4,
        WriteOnly,
        Zero,
        0,
        0xffff_ffff
    ),
    r!(IResult, "I_RESULT", 0x224, 4, ReadOnly, Zero, 0, 0),
    r!(IDetail0, "I_DETAIL0", 0x228, 8, ReadOnly, Zero, 0, 0),
    r!(IDetail1, "I_DETAIL1", 0x230, 8, ReadOnly, Zero, 0, 0),
    r!(IDetail2, "I_DETAIL2", 0x238, 8, ReadOnly, Zero, 0, 0),
    r!(IRspEpoch, "I_RSP_EPOCH", 0x240, 8, ReadOnly, Zero, 0, 0),
    r!(
        IRspSourceId,
        "I_RSP_SOURCE_ID",
        0x248,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        IRspSourceKind,
        "I_RSP_SOURCE_KIND",
        0x250,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(IRspState, "I_RSP_STATE", 0x254, 4, ReadOnly, Zero, 0, 0),
    r!(
        IRspRouteKind,
        "I_RSP_ROUTE_KIND",
        0x258,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        IRspNotificationKind,
        "I_RSP_NOTIFICATION_KIND",
        0x25c,
        4,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(IRspVector, "I_RSP_VECTOR", 0x260, 8, ReadOnly, Zero, 0, 0),
    r!(
        IRspMessageId,
        "I_RSP_MESSAGE_ID",
        0x268,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
    r!(
        IRspPendingMessages,
        "I_RSP_PENDING_MESSAGES",
        0x270,
        8,
        ReadOnly,
        Zero,
        0,
        0
    ),
];
