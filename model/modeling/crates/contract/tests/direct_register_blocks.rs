//! Generated direct-register geometry and permission contract.

use hardware_interface_schema::register::Access;
use hardware_interface_schema::{RegisterLayout, register_map};
use pdma_model_contract::abi::registers::{
    GlobalRegisters, IrqSourceRegisters, PdmaRegisterMap, QueueInstanceRegisters, QueueRegisters,
    QueueRegistersRegisterId, RegisterAddressError, RegisterLocation, irq_offset, locate,
    queue_offset,
};

register_map! { map TwoQueues {
    alignment: 8,
    placements: [queues: QueueInstanceRegisters { count: 2 }],
} }

#[test]
fn nested_write_isolates_one_source_in_one_queue() {
    // Given
    let mut values = TwoQueuesValues::default();
    let mut expected = values;
    expected.queues[1].sources[1].route_vector =
        pdma_model_contract::abi::registers::irq::IRouteVectorValues::new(71);
    let offset = TwoQueues::QUEUES.stride()
        + QueueInstanceRegisters::SOURCES.offset()
        + QueueInstanceRegisters::SOURCES.stride()
        + IrqSourceRegisters::ROUTE_VECTOR.offset();

    // When
    let result = TwoQueues::write_register(&mut values, offset, 71);

    // Then
    assert_eq!(result, Some(()));
    assert_eq!(values, expected);
}

#[test]
fn source_reset_keeps_delivery_masked() {
    // Given
    let values = pdma_model_contract::abi::registers::IrqSourceRegistersValues::default();

    // When
    let masked = IrqSourceRegisters::read_register(&values, IrqSourceRegisters::MASKED.offset());

    // Then
    assert_eq!(masked, Some(1));
}

#[test]
fn inferred_blocks_do_not_overlap() {
    // Given
    let root = PdmaRegisterMap::resolve().expect("valid generated composition");

    // When
    let overlap = root
        .registers()
        .iter()
        .zip(root.registers().iter().skip(1))
        .any(|(left, right)| left.offset() + left.definition().width() > right.offset());

    // Then
    assert!(!overlap);
    assert_eq!(GlobalRegisters::EXTENT, 60);
    assert_eq!(QueueRegisters::EXTENT, 112);
    assert_eq!(IrqSourceRegisters::EXTENT, 40);
    assert_eq!(QueueInstanceRegisters::EXTENT, 192);
    assert_eq!(PdmaRegisterMap::QUEUES.offset(), 64);
    assert_eq!(PdmaRegisterMap::EXTENT, 256);
}

#[test]
fn device_cursors_have_read_only_metadata_at_high_queue_index() {
    for local in [
        QueueRegisters::SQ_CAPTURED.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
    ] {
        // Given
        let offset = queue_offset(1_000_000, local).expect("dynamic queue address");

        // When
        let location = locate(offset).expect("exact device cursor");

        // Then
        let RegisterLocation::Queue { index, register } = location else {
            panic!("expected queue register");
        };
        assert_eq!(index, 1_000_000);
        assert_eq!(register.offset(), local);
        assert_eq!(register.access(), Access::ReadOnly);
        assert_eq!(register.writable_mask(), 0);
    }
}

#[test]
fn dynamic_sources_preserve_exact_start_and_permissions() {
    // Given
    let offset =
        irq_offset(90_000, 1, IrqSourceRegisters::ACK.offset()).expect("dynamic event source");

    // When
    let location = locate(offset).expect("exact acknowledge register");

    // Then
    let RegisterLocation::Irq {
        index,
        source_index,
        register,
    } = location
    else {
        panic!("expected IRQ source register");
    };
    assert_eq!((index, source_index), (90_000, 1));
    assert_eq!(register.access(), Access::WriteOnly);
    assert_eq!(register.writable_mask(), 1);
    assert!(locate(offset + 1).is_none());
    assert!(locate(offset + 2).is_none());
}

#[test]
fn address_helpers_reject_overflow_and_invalid_local_geometry() {
    // Given
    let local = QueueRegisters::SQ_BASE.offset();

    // When
    let overflow = queue_offset(u32::MAX, local);
    let source = irq_offset(0, 2, IrqSourceRegisters::ACK.offset());
    let hole = queue_offset(0, QueueRegisters::SQ_DEPTH.offset() + 4);

    // Then
    assert_eq!(overflow, Err(RegisterAddressError::AddressOverflow));
    assert_eq!(source, Err(RegisterAddressError::SourceIndex { index: 2 }));
    assert_eq!(hole, Err(RegisterAddressError::LocalOffset { offset: 12 }));
}

#[test]
fn checked_addresses_round_trip_at_last_representable_instance() {
    // Given
    let stride = u32::try_from(QueueInstanceRegisters::EXTENT).expect("small stride");
    let base = u32::try_from(PdmaRegisterMap::QUEUES.offset()).expect("small base");
    let index = (u32::MAX - base) / stride;

    // When
    let last = queue_offset(index, QueueRegisters::SQ_BASE.offset()).expect("last base fits");

    // Then
    assert!(
        matches!(locate(last), Some(RegisterLocation::Queue { index: found, register })
        if found == index && register.id() == QueueRegistersRegisterId::SqBase)
    );
    assert_eq!(
        queue_offset(index + 1, 0),
        Err(RegisterAddressError::AddressOverflow)
    );
    assert_eq!(
        irq_offset(index, 1, IrqSourceRegisters::PENDING_MESSAGES.offset()),
        Ok(u32::MAX - 7)
    );
}
