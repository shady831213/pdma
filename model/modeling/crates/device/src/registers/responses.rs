use crate::interface::mmio::queue::complete;
use crate::registers::PdmaRegisters;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueResponseBank, RegisterId, ResponseUpdate,
};

fn completion(result: MmioResult, details: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails { result, details }
}

#[test]
fn command_completion_replaces_or_preserves_atomically() {
    let registers = PdmaRegisters::new();
    registers.view_mut(|mut root| {
        complete(
            &mut root,
            completion(MmioResult::Ok, [1, 2, 3]),
            ResponseUpdate::Replace(QueueResponseBank {
                epoch: 0xfeed_u64.into(),
                ..QueueResponseBank::default()
            }),
        );
        complete(
            &mut root,
            completion(MmioResult::InvalidTransaction, [4, 5, 6]),
            ResponseUpdate::Preserve,
        );
    });

    assert_eq!(
        registers.read(RegisterId::QResult),
        MmioResult::InvalidTransaction.code().to_le_bytes()
    );
    assert_eq!(
        registers.read(RegisterId::QRspEpoch),
        0xfeed_u64.to_le_bytes()
    );
}

#[test]
fn empty_completion_zero_fills_the_response() {
    let registers = PdmaRegisters::new();
    registers.view_mut(|mut root| {
        complete(
            &mut root,
            completion(MmioResult::Ok, [0; 3]),
            ResponseUpdate::Replace(QueueResponseBank {
                epoch: 0xfeed_u64.into(),
                ..QueueResponseBank::default()
            }),
        );
        complete(
            &mut root,
            completion(MmioResult::Empty, [0; 3]),
            ResponseUpdate::Empty,
        );
    });

    assert_eq!(
        registers.read(RegisterId::QResult),
        MmioResult::Empty.code().to_le_bytes()
    );
    assert_eq!(registers.read(RegisterId::QRspEpoch), 0_u64.to_le_bytes());
}
