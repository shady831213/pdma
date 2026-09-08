use super::*;
use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector};

#[test]
fn owner_irq_request_snapshot_reads_all_five_selector_fields_once() {
    // Given: five distinct selector values installed through the IRQ owner's bank.
    let mut controller =
        PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0)));
    let registers = &mut controller.registers;
    registers.expected_epoch = 11.into();
    registers.source_id = 12.into();
    registers.source_kind = 1.into();
    registers.route_kind = 1.into();
    registers.route_vector = 13.into();

    // When: the IRQ request is captured from the IRQ owner.
    let request = controller.snapshot_request();

    // Then: the owned snapshot contains every selector and route field exactly.
    assert_eq!(
        (
            request.expected_epoch,
            request.source_id,
            request.source_kind,
            request.route_kind,
            request.route_vector,
        ),
        (11, 12, 1, 1, 13)
    );
}

#[test]
fn owner_irq_effect_empties_result_details_and_response_atomically() {
    // Given: an IRQ response bank whose result, details, and lanes are all nonzero.
    let mut controller =
        PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0)));
    let registers = &mut controller.registers;
    registers.result = 1.into();
    registers.detail0 = 2.into();
    registers.detail1 = 3.into();
    registers.detail2 = 4.into();
    registers.response.epoch = 5.into();
    registers.response.source_id = 6.into();
    registers.response.source_kind = 7.into();
    registers.response.state = 8.into();
    registers.response.route_kind = 9.into();
    registers.response.notification_kind = 10.into();
    registers.response.vector = 11.into();
    registers.response.message_id = 12.into();
    registers.response.pending_messages = 13.into();

    // When: one typed IRQ effect publishes Empty and distinct diagnostic details.
    controller.complete(
        MmioResultDetails {
            result: MmioResult::Empty,
            details: [14, 15, 16],
        },
        ResponseUpdate::Empty,
    );

    let registers = &controller.registers;
    // Then: raw consumers see the new result/details and all response lanes cleared together.
    assert_eq!(
        [
            registers.result.raw().to_le_bytes().to_vec(),
            registers.detail0.raw().to_le_bytes().to_vec(),
            registers.detail1.raw().to_le_bytes().to_vec(),
            registers.detail2.raw().to_le_bytes().to_vec(),
            registers.response.epoch.raw().to_le_bytes().to_vec(),
            registers.response.source_id.raw().to_le_bytes().to_vec(),
            registers.response.source_kind.raw().to_le_bytes().to_vec(),
            registers.response.state.raw().to_le_bytes().to_vec(),
            registers.response.route_kind.raw().to_le_bytes().to_vec(),
            registers
                .response
                .notification_kind
                .raw()
                .to_le_bytes()
                .to_vec(),
            registers.response.vector.raw().to_le_bytes().to_vec(),
            registers.response.message_id.raw().to_le_bytes().to_vec(),
            registers
                .response
                .pending_messages
                .raw()
                .to_le_bytes()
                .to_vec(),
        ],
        [
            2_u32.to_le_bytes().to_vec(),
            14_u64.to_le_bytes().to_vec(),
            15_u64.to_le_bytes().to_vec(),
            16_u64.to_le_bytes().to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 4].to_vec(),
            [0_u8; 4].to_vec(),
            [0_u8; 4].to_vec(),
            [0_u8; 4].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
        ]
    );
}
