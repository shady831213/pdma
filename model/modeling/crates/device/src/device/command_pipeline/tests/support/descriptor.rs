use super::{Case, Fixture};
use crate::test_support;
use pdma_model_contract::abi::DmaRegion;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadStridedCopyValues,
    AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues, U32WordValues,
};

pub(super) fn command(fixture: &Fixture, case: Case, [a, b, x, y]: [DmaRegion; 4]) -> AbiCommand {
    let payload = match case {
        Case::Copy => return test_support::copy(7, a.base().raw(), x.base().raw(), 4),
        Case::Strided => AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
            source_dma_address: a.base().raw(),
            destination_dma_address: x.base().raw(),
            rows: U32WordValues { value: 2 },
            row_bytes: U32WordValues { value: 2 },
            source_stride: U32WordValues { value: 3 },
            destination_stride: U32WordValues { value: 3 },
        }),
        Case::Broadcast => {
            let list = crate::test_support::allocate(&fixture.allocator, 16);
            let entries = [
                x.base().raw().to_le_bytes(),
                (y.base().raw() + 2).to_le_bytes(),
            ]
            .concat();
            crate::memory::write(&fixture.core.memory, list.base(), &entries)
                .expect("destinations");
            AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
                source_dma_address: a.base().raw() + 1,
                destination_list_dma_address: list.base().raw(),
                length: U32WordValues { value: 3 },
                destination_count: U32WordValues { value: 2 },
            })
        }
        Case::VectorAdd => AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
            source_a_dma_address: a.base().raw(),
            source_b_dma_address: b.base().raw(),
            destination_dma_address: x.base().raw(),
            length: U32WordValues { value: 4 },
        }),
        Case::UnknownBuffer => return test_support::copy(7, 0xdead_0000, x.base().raw(), 4),
        Case::RangeOverflow => return test_support::copy(7, u64::MAX, x.base().raw(), 2),
        Case::OutOfBounds => return test_support::copy(7, a.base().raw(), x.base().raw(), 9),
    };
    AbiCommand::from_payload(7, payload)
}
