use crate::operation::{
    BroadcastOperands, CopyOperands, DecodedCommand, DecodedOperation, StridedOperands,
    VectorOperands,
};
use pdma_model_contract::abi::{AbiCommand, AbiCommandPayloadValues, DmaAddress};
use pdma_model_contract::semantic::CommandId;

pub(crate) const fn decode(command: AbiCommand, id: CommandId) -> DecodedCommand {
    let operation = match command.payload() {
        AbiCommandPayloadValues::Copy(value) => DecodedOperation::Copy(CopyOperands {
            source: DmaAddress::new(value.source_dma_address),
            destination: DmaAddress::new(value.destination_dma_address),
            length: value.length.value(),
        }),
        AbiCommandPayloadValues::StridedCopy(value) => {
            DecodedOperation::StridedCopy(StridedOperands {
                source: DmaAddress::new(value.source_dma_address),
                destination: DmaAddress::new(value.destination_dma_address),
                rows: value.rows.value(),
                row_bytes: value.row_bytes.value(),
                source_stride: value.source_stride.value(),
                destination_stride: value.destination_stride.value(),
            })
        }
        AbiCommandPayloadValues::BroadcastCopy(value) => {
            DecodedOperation::BroadcastCopy(BroadcastOperands {
                source: DmaAddress::new(value.source_dma_address),
                destination_list: DmaAddress::new(value.destination_list_dma_address),
                destination_count: value.destination_count.value(),
                length: value.length.value(),
            })
        }
        AbiCommandPayloadValues::VectorAdd(value) => DecodedOperation::VectorAdd(VectorOperands {
            source_a: DmaAddress::new(value.source_a_dma_address),
            source_b: DmaAddress::new(value.source_b_dma_address),
            destination: DmaAddress::new(value.destination_dma_address),
            length: value.length.value(),
        }),
    };
    DecodedCommand { id, operation }
}
