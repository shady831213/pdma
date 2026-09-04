use super::*;
use crate::registers::PdmaRegisters;
use pdma_model_contract::abi::mmio::{MmioResult, RegisterId};

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}

#[test]
fn typed_root_irq_request_snapshot_reads_all_five_selector_fields_once() {
    // Given: five distinct selector values installed through the generated IRQ root.
    let registers = PdmaRegisters::new();
    registers.view_mut(|mut root| {
        let mut irq = invariant(root.irq_mut());
        invariant(invariant(irq.expected_epoch_mut()).set_value(11));
        invariant(invariant(irq.source_id_mut()).set_value(12));
        invariant(invariant(irq.source_kind_mut()).set_value(1));
        invariant(invariant(irq.route_kind_mut()).set_value(1));
        invariant(invariant(irq.route_vector_mut()).set_value(13));
    });

    // When: the IRQ request is captured from one generated group view.
    let request = snapshot_request(&registers);

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
fn typed_root_irq_effect_empties_result_details_and_response_atomically() {
    // Given: a generated IRQ response group whose result, details, and lanes are all nonzero.
    let registers = PdmaRegisters::new();
    registers.view_mut(|mut root| {
        let mut irq = invariant(root.irq_mut());
        invariant(invariant(irq.result_mut()).set_value(1));
        invariant(invariant(irq.detail0_mut()).set_value(2));
        invariant(invariant(irq.detail1_mut()).set_value(3));
        invariant(invariant(irq.detail2_mut()).set_value(4));
        invariant(invariant(irq.rsp_epoch_mut()).set_value(5));
        invariant(invariant(irq.rsp_source_id_mut()).set_value(6));
        invariant(invariant(irq.rsp_source_kind_mut()).set_value(7));
        invariant(invariant(irq.rsp_state_mut()).set_value(8));
        invariant(invariant(irq.rsp_route_kind_mut()).set_value(9));
        invariant(invariant(irq.rsp_notification_kind_mut()).set_value(10));
        invariant(invariant(irq.rsp_vector_mut()).set_value(11));
        invariant(invariant(irq.rsp_message_id_mut()).set_value(12));
        invariant(invariant(irq.rsp_pending_messages_mut()).set_value(13));
    });

    // When: one typed IRQ effect publishes Empty and distinct diagnostic details.
    registers.view_mut(|mut root| {
        complete(
            &mut root,
            MmioResultDetails {
                result: MmioResult::Empty,
                details: [14, 15, 16],
            },
            ResponseUpdate::Empty,
        );
    });

    // Then: raw consumers see the new result/details and all response lanes cleared together.
    assert_eq!(
        [
            RegisterId::IResult,
            RegisterId::IDetail0,
            RegisterId::IDetail1,
            RegisterId::IDetail2,
            RegisterId::IRspEpoch,
            RegisterId::IRspSourceId,
            RegisterId::IRspSourceKind,
            RegisterId::IRspState,
            RegisterId::IRspRouteKind,
            RegisterId::IRspNotificationKind,
            RegisterId::IRspVector,
            RegisterId::IRspMessageId,
            RegisterId::IRspPendingMessages,
        ]
        .map(|register| registers.read(register)),
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
