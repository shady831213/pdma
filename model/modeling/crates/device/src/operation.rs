use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::{CommandId, TraceOperation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedCommand {
    pub(crate) id: CommandId,
    pub(crate) operation: DecodedOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedOperation {
    Copy(CopyOperands),
    StridedCopy(StridedOperands),
    BroadcastCopy(BroadcastOperands),
    VectorAdd(VectorOperands),
}

impl DecodedOperation {
    pub(crate) const fn trace_operation(&self) -> TraceOperation {
        match self {
            Self::Copy(_) => TraceOperation::Copy,
            Self::StridedCopy(_) => TraceOperation::StridedCopy,
            Self::BroadcastCopy(_) => TraceOperation::BroadcastCopy,
            Self::VectorAdd(_) => TraceOperation::VectorAdd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyOperands {
    pub(crate) source: DmaAddress,
    pub(crate) destination: DmaAddress,
    pub(crate) length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StridedOperands {
    pub(crate) source: DmaAddress,
    pub(crate) destination: DmaAddress,
    pub(crate) rows: u32,
    pub(crate) row_bytes: u32,
    pub(crate) source_stride: u32,
    pub(crate) destination_stride: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BroadcastOperands {
    pub(crate) source: DmaAddress,
    pub(crate) destination_list: DmaAddress,
    pub(crate) destination_count: u32,
    pub(crate) length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VectorOperands {
    pub(crate) source_a: DmaAddress,
    pub(crate) source_b: DmaAddress,
    pub(crate) destination: DmaAddress,
    pub(crate) length: u32,
}
