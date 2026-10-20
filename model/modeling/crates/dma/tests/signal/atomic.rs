use super::support::{Fixture, Result, SIGNAL};
use pdma_model_contract::dma::{
    Command, SignalCommand, SignalOperation, SignalOrder, SignalTarget, WaitCommand, WaitCondition,
    WaitOrder,
};

#[test]
fn every_signal_operation_preserves_old_bits_and_applies_its_update() -> Result {
    for order in [
        SignalOrder::Relaxed,
        SignalOrder::Acquire,
        SignalOrder::Release,
        SignalOrder::AcquireRelease,
    ] {
        for (operation, initial, value, expected, final_value) in [
            (SignalOperation::Load, 13, 99, 0, 13),
            (SignalOperation::Store, 13, 99, 0, 99),
            (SignalOperation::Add, u64::MAX, 2, 0, 1),
            (SignalOperation::Subtract, 0, 1, 0, u64::MAX),
            (SignalOperation::And, 0b1100, 0b1010, 0, 0b1000),
            (SignalOperation::Or, 0b1100, 0b1010, 0, 0b1110),
            (SignalOperation::Xor, 0b1100, 0b1010, 0, 0b0110),
            (SignalOperation::Exchange, 13, u64::MAX, 0, u64::MAX),
            (SignalOperation::CompareExchange, 13, 99, 13, 99),
            (SignalOperation::CompareExchange, 13, 99, 14, 13),
        ] {
            if !operation.allows_order(order) {
                continue;
            }
            // Given: a full-width operand and a legal operation/order combination.
            let mut fixture = Fixture::new()?;
            fixture.store(initial)?;
            fixture.submit(
                0,
                &[Command::signal(
                    91,
                    SignalCommand {
                        target: SignalTarget {
                            address: SIGNAL,
                            context: 1,
                        },
                        operation,
                        order,
                        value,
                        expected,
                    },
                )],
            )?;
            // When: real hardware processes the descriptor and continues polling.
            fixture.tick(256)?;
            // Then: one response reports old bits and the memory holds exactly one update.
            assert_eq!(fixture.produced(0)?, 1, "{operation:?}/{order:?}");
            assert_eq!(fixture.value()?, final_value, "{operation:?}/{order:?}");
            assert_eq!(
                fixture.response(0, 0)?.signal_value(),
                Some(i64::from_le_bytes(initial.to_le_bytes())),
                "{operation:?}/{order:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn signed_wait_predicates_respect_negative_and_extreme_values() -> Result {
    for order in [WaitOrder::Relaxed, WaitOrder::Acquire] {
        for (condition, initial, compare, satisfied) in [
            (WaitCondition::Equal, -1_i64, i64::MIN, i64::MIN),
            (WaitCondition::NotEqual, i64::MIN, i64::MIN, i64::MAX),
            (WaitCondition::LessThan, i64::MAX, 0, i64::MIN),
            (WaitCondition::GreaterOrEqual, i64::MIN, -1, i64::MAX),
        ] {
            // Given: a signed comparison initially false, spanning the sign boundary.
            let mut fixture = Fixture::new()?;
            fixture.store(u64::from_le_bytes(initial.to_le_bytes()))?;
            fixture.submit(
                0,
                &[Command::wait(
                    92,
                    WaitCommand {
                        target: SignalTarget {
                            address: SIGNAL,
                            context: 1,
                        },
                        condition,
                        order,
                        compare: u64::from_le_bytes(compare.to_le_bytes()),
                    },
                )],
            )?;
            // When: hardware waits, then software makes the predicate true.
            fixture.tick(128)?;
            assert_eq!(fixture.produced(0)?, 0, "{condition:?}/{order:?}");
            fixture.store(u64::from_le_bytes(satisfied.to_le_bytes()))?;
            fixture.tick(128)?;
            // Then: completion reports the satisfying signed value without modifying it.
            assert_eq!(fixture.produced(0)?, 1);
            assert_eq!(fixture.response(0, 0)?.signal_value(), Some(satisfied));
            assert_eq!(
                fixture.value()?,
                u64::from_le_bytes(satisfied.to_le_bytes())
            );
        }
    }
    Ok(())
}
