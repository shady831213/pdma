use super::IrqSourceState;
use crate::queue::exact::PdmaControlError;
use pdma_model_contract::abi::registers::IrqSourceRegisters as Registers;
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqMessage, IrqMessageId, IrqSource, IrqVector,
};

const SOURCE: IrqSource = IrqSource::Completion(CompletionRingId::new(7));
const EPOCH: DeviceEpoch = DeviceEpoch::initial();

fn reg(
    placement: hardware_interface_schema::register_map::RegisterMapPlacement<
        pdma_model_contract::abi::registers::IrqSourceRegistersRegisterId,
    >,
) -> hardware_interface_schema::register::RegisterSpec<
    pdma_model_contract::abi::registers::IrqSourceRegistersRegisterId,
> {
    Registers::register(placement.id()).expect("source register")
}

fn enabled(message: bool, masked: bool) -> IrqSourceState {
    let mut state = IrqSourceState::default();
    state
        .write_register(reg(Registers::ROUTE_KIND), u64::from(message), false)
        .expect("route");
    state
        .write_register(reg(Registers::ROUTE_VECTOR), 5, false)
        .expect("vector");
    state
        .write_register(reg(Registers::ENABLED), 1, false)
        .expect("enable");
    state
        .write_register(reg(Registers::MASKED), u64::from(masked), false)
        .expect("mask");
    state
}

#[test]
fn enabled_unmasked_latch_asserts_line() {
    // Given: an enabled unmasked line source.
    let mut state = enabled(false, false);
    // When: a record publication latches its source.
    assert!(state.latch());
    // Then: the pending source asserts its line.
    assert!(state.has_line_cause());
    assert!(state.asserts_line());
}

#[test]
fn masked_latch_preserves_cause_without_line_assertion() {
    // Given: an enabled masked line source.
    let mut state = enabled(false, true);
    // When: a record publication latches its source.
    assert!(state.latch());
    // Then: the pending cause remains masked.
    assert!(state.has_line_cause());
    assert!(!state.asserts_line());
}

#[test]
fn disabled_latch_preserves_reset_state() {
    // Given: a source at reset.
    let mut state = IrqSourceState::default();
    // When: publication attempts to latch the disabled source.
    assert!(!state.latch());
    // Then: the source retains reset registers and empty notification state.
    assert_eq!(state, IrqSourceState::default());
}

#[test]
fn ack_clears_drained_cause_and_preserves_mask() {
    // Given: a masked pending source.
    let mut state = enabled(false, true);
    state.latch();
    // When: software acknowledges after draining the records.
    state
        .write_register(reg(Registers::ACK), 1, false)
        .expect("ack");
    // Then: the cause clears while mask and enable remain configured.
    assert!(!state.has_line_cause());
    assert_eq!(
        state.read_register(Registers::MASKED.offset(), false),
        Some(1)
    );
    assert_eq!(
        state.read_register(Registers::ENABLED.offset(), false),
        Some(1)
    );
}

#[test]
fn ack_preserves_line_cause_with_unread_records() {
    // Given: an asserted source with unread records.
    let mut state = enabled(false, false);
    state.latch();
    // When: software acknowledges without draining the records.
    state
        .write_register(reg(Registers::ACK), 1, true)
        .expect("ack");
    // Then: the existing cause continues asserting the line.
    assert!(state.asserts_line());
}

#[test]
fn disabling_pending_source_preserves_cause() {
    // Given: an asserted line source.
    let mut state = enabled(false, false);
    state.latch();
    // When: software disables the source.
    state
        .write_register(reg(Registers::ENABLED), 0, false)
        .expect("disable");
    // Then: the cause remains pending without delivery.
    assert!(state.has_line_cause());
    assert!(!state.asserts_line());
}

#[test]
fn remasking_pending_source_preserves_cause() {
    // Given: an asserted line source.
    let mut state = enabled(false, false);
    state.latch();
    // When: software masks the source.
    state
        .write_register(reg(Registers::MASKED), 1, false)
        .expect("mask");
    // Then: the cause remains pending without delivery.
    assert!(state.has_line_cause());
    assert!(!state.asserts_line());
}

#[test]
fn message_triggers_keep_distinct_ordered_identities() {
    // Given: an enabled unmasked message source.
    let mut state = enabled(true, false);
    // When: two publications latch the source before delivery.
    state.latch();
    state.latch();
    // Then: delivery consumes two distinct message identities in order.
    for id in 0..2 {
        assert_eq!(
            state.take_message(EPOCH, SOURCE),
            Some(IrqMessage::new(
                IrqMessageId::new(id),
                IrqVector::new(5),
                EpochIrqSource::new(EPOCH, SOURCE),
            ))
        );
    }
    assert_eq!(state.take_message(EPOCH, SOURCE), None);
}

#[test]
fn unmask_releases_all_retained_message_triggers() {
    // Given: two pending messages held by the source mask.
    let mut state = enabled(true, true);
    state.latch();
    state.latch();
    assert_eq!(state.take_message(EPOCH, SOURCE), None);
    // When: software unmasks the source.
    state
        .write_register(reg(Registers::MASKED), 0, false)
        .expect("unmask");
    // Then: both triggers remain available for delivery.
    assert!(state.take_message(EPOCH, SOURCE).is_some());
    assert!(state.take_message(EPOCH, SOURCE).is_some());
    assert_eq!(state.take_message(EPOCH, SOURCE), None);
}

#[test]
fn active_route_write_rejection_preserves_source() {
    // Given: sources active by enable, pending line cause, or retained message.
    for pending in 0..3 {
        let mut state = enabled(pending == 2, true);
        if pending != 0 {
            state.latch();
            state
                .write_register(reg(Registers::ENABLED), 0, false)
                .expect("disable");
        }
        let before = state;
        // When: software attempts either route register while the source is active.
        for register in [Registers::ROUTE_KIND, Registers::ROUTE_VECTOR] {
            assert_eq!(
                state.write_register(reg(register), 1, false),
                Err(PdmaControlError::InvalidTransaction)
            );
        }
        // Then: rejected writes leave the complete source state unchanged.
        assert_eq!(state, before);
    }
}

#[test]
fn message_counts_and_identities_saturate_at_their_limit() {
    // Given: a source at the maximum modeled count and identity.
    let mut state = enabled(true, false);
    state.pending_messages = usize::MAX;
    state.next_message_id = usize::MAX;
    // When: another publication latches before one message is delivered.
    state.latch();
    let message = state.take_message(EPOCH, SOURCE).expect("message");
    // Then: neither counter wraps to zero.
    assert_eq!(message.id().raw(), usize::MAX);
    assert_eq!(state.pending_messages, usize::MAX - 1);
    assert_eq!(state.next_message_id, usize::MAX);
}
