#![doc = "Scalar selector and atomic operand wire boundaries."]
use pdma_model_contract::dma::{
    AtomicCommand, AtomicCommandOperandsValues, AtomicCommandOperation, AtomicCommandOrder,
    WaitCommand, WaitCommandComparison, WordTargetWidth,
};

#[test]
fn atomic_operations_decode_all_orders_and_widths_without_result_storage() {
    // Given: independently encoded operations with width-specific target metadata.
    let operations = [
        AtomicCommandOperation::Add,
        AtomicCommandOperation::Subtract,
        AtomicCommandOperation::And,
        AtomicCommandOperation::Or,
        AtomicCommandOperation::Xor,
        AtomicCommandOperation::Exchange,
        AtomicCommandOperation::CompareExchange,
    ];
    let orders = [
        AtomicCommandOrder::Relaxed,
        AtomicCommandOrder::Acquire,
        AtomicCommandOrder::Release,
        AtomicCommandOrder::AcquireRelease,
    ];
    for (op, operation) in operations.into_iter().enumerate() {
        for (order, ordering) in orders.into_iter().enumerate() {
            for (width, target_width) in [WordTargetWidth::Bits32, WordTargetWidth::Bits64]
                .into_iter()
                .enumerate()
            {
                let mut bytes = [0_u8; 40];
                bytes[0] = 0x80;
                bytes[8] = 9;
                bytes[12] = u8::try_from(width).unwrap();
                bytes[16] = u8::try_from(order).unwrap();
                bytes[20] = u8::try_from(op).unwrap();
                bytes[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
                if op == 6 {
                    bytes[32] = 31;
                }
                // When: the hardware decodes an atomic payload.
                let decoded = AtomicCommand::from_bytes(&bytes).unwrap();
                // Then: operation, ordering and complete operand bits survive without a result address.
                assert_eq!(decoded.operation, operation);
                assert_eq!(decoded.order, ordering);
                assert_eq!(decoded.target.width, target_width);
                match decoded.operands {
                    AtomicCommandOperandsValues::Add(value) => assert_eq!(value.value, u64::MAX),
                    AtomicCommandOperandsValues::Subtract(value) => {
                        assert_eq!(value.value, u64::MAX);
                    }
                    AtomicCommandOperandsValues::And(value) => assert_eq!(value.value, u64::MAX),
                    AtomicCommandOperandsValues::Or(value) => assert_eq!(value.value, u64::MAX),
                    AtomicCommandOperandsValues::Xor(value) => assert_eq!(value.value, u64::MAX),
                    AtomicCommandOperandsValues::Exchange(value) => {
                        assert_eq!(value.value, u64::MAX);
                    }
                    AtomicCommandOperandsValues::CompareExchange(value) => {
                        assert_eq!((value.compare, value.value), (u64::MAX, 31));
                    }
                }
                assert_eq!(decoded.to_bytes().unwrap(), bytes);
            }
        }
    }
}

#[test]
fn wait_comparisons_decode_all_assigned_signed_and_unsigned_predicates() {
    // Given: the assigned comparison tags, including the modular reached predicate.
    let predicates = [
        WaitCommandComparison::Equal,
        WaitCommandComparison::NotEqual,
        WaitCommandComparison::UnsignedLess,
        WaitCommandComparison::UnsignedLessEqual,
        WaitCommandComparison::UnsignedGreater,
        WaitCommandComparison::UnsignedGreaterEqual,
        WaitCommandComparison::SignedLess,
        WaitCommandComparison::SignedLessEqual,
        WaitCommandComparison::SignedGreater,
        WaitCommandComparison::SignedGreaterEqual,
        WaitCommandComparison::Reached,
    ];
    for (tag, predicate) in predicates.into_iter().enumerate() {
        let mut bytes = [0_u8; 32];
        bytes[0] = 0x80;
        bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        bytes[24] = u8::try_from(tag).unwrap();
        // When: the wire decoder selects a comparison.
        let decoded = WaitCommand::from_bytes(&bytes).unwrap();
        // Then: signedness and comparison kind remain explicit in the typed payload.
        assert_eq!(decoded.comparison, predicate);
        assert_eq!(decoded.value, u64::MAX);
        assert_eq!(decoded.to_bytes().unwrap(), bytes);
    }
}
