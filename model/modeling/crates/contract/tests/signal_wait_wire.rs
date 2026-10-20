//! Tagged command and signed signal wire behavior.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{
    Command, CommandOpcode, Response, ResponseInfoValues, SignalCommand, SignalErrorInfo,
    SignalErrorReason, SignalInfo, SignalOperation, SignalOrder, SignalTarget, WaitCommand,
    WaitCondition, WaitOrder,
};

#[test]
fn signal_and_wait_round_trip_signed_values() -> Result<(), SchemaCodecError> {
    // Given: a negative signal value and a signed wait threshold.
    let target = SignalTarget {
        address: 0x1200,
        context: 3,
    };
    let signal = Command::signal(
        9,
        SignalCommand {
            target,
            operation: SignalOperation::Add,
            order: SignalOrder::AcquireRelease,
            value: u64::from_le_bytes((-7_i64).to_le_bytes()),
            expected: 0,
        },
    );
    let wait = Command::wait(
        10,
        WaitCommand {
            target,
            condition: WaitCondition::LessThan,
            order: WaitOrder::Acquire,
            compare: u64::from_le_bytes((-2_i64).to_le_bytes()),
        },
    );
    // When: commands cross their generated wire boundary.
    let signal_bytes = signal.to_bytes()?;
    let wait_bytes = wait.to_bytes()?;
    // Then: tags select distinct records and signed operands survive unchanged.
    assert_eq!(signal_bytes.len(), 112);
    assert_eq!(signal_bytes[12..16], 1_u32.to_le_bytes());
    assert_eq!(wait_bytes[12..16], 2_u32.to_le_bytes());
    assert_eq!(signal.opcode, CommandOpcode::Signal);
    assert_eq!(Command::from_bytes(&signal_bytes)?, signal);
    assert_eq!(Command::from_bytes(&wait_bytes)?, wait);
    assert!(WaitCondition::LessThan.matches(-7, -2));
    assert!(!WaitCondition::LessThan.matches(7, -2));
    Ok(())
}

#[test]
fn signal_responses_preserve_observed_value_and_error_reason() -> Result<(), SchemaCodecError> {
    // Given: an observed negative signal and a backend rejection.
    let success = ResponseInfoValues::SignalSuccess(SignalInfo { value: u64::MAX });
    let error = ResponseInfoValues::SignalError(SignalErrorInfo {
        reason: SignalErrorReason::Unsupported,
    });
    for (info, tag, value, halted) in [(success, 7, Some(-1), false), (error, 8, None, true)] {
        let response = Response {
            id: 42,
            status: info.tag(),
            info,
            ..Response::ZERO
        };
        // When: software decodes the terminal response.
        let bytes = response.to_bytes()?;
        let decoded = Response::from_bytes(&bytes)?;
        // Then: signal results retain their own meaning and never report copy progress.
        assert_eq!(bytes.len(), 24);
        assert_eq!(bytes[8], tag);
        assert_eq!(decoded.signal_value(), value);
        assert_eq!(decoded.status.halts(), halted);
        assert_eq!(decoded.bytes_completed(), 0);
        assert_eq!(decoded, response);
    }
    Ok(())
}

#[test]
fn signal_operation_orders_and_signed_wait_predicates_follow_the_contract() {
    // Given: the four memory orders and signed values crossing zero.
    let orders = [
        SignalOrder::Relaxed,
        SignalOrder::Acquire,
        SignalOrder::Release,
        SignalOrder::AcquireRelease,
    ];
    // When: each operation checks the orders it can use.
    let load = orders.map(|order| SignalOperation::Load.allows_order(order));
    let store = orders.map(|order| SignalOperation::Store.allows_order(order));
    // Then: load/store exclude invalid orders and comparisons use signed arithmetic.
    assert_eq!(load, [true, true, false, false]);
    assert_eq!(store, [true, false, true, false]);
    for operation in [
        SignalOperation::Add,
        SignalOperation::Subtract,
        SignalOperation::And,
        SignalOperation::Or,
        SignalOperation::Xor,
        SignalOperation::Exchange,
        SignalOperation::CompareExchange,
    ] {
        assert!(
            orders
                .into_iter()
                .all(|order| operation.allows_order(order))
        );
    }
    assert!(WaitCondition::Equal.matches(-1, -1));
    assert!(WaitCondition::NotEqual.matches(-1, 0));
    assert!(WaitCondition::GreaterOrEqual.matches(0, -1));
    assert!(!WaitCondition::GreaterOrEqual.matches(i64::MIN, i64::MAX));
}
