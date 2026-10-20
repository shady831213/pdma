use super::memory;
use crate::TestResult;
use hardware_interface_runtime::{
    AtomicOrder, AtomicReadOrder, AtomicWriteOrder, MemoryAccess, MemoryAlignment,
};
use pdma_model_software::signal::Signal;

#[test]
fn loads_and_stores_preserve_signed_bit_patterns() -> TestResult {
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        // Given: a word initialized through ordinary software memory access.
        let memory = memory()?;
        memory.write(0x1000.into(), MemoryAlignment::ONE, &value.to_le_bytes())?;
        let signal = Signal::new(&memory, 0x1000.into());
        // When: an acquire load and release store operate on signed values.
        let observed = signal.load(AtomicReadOrder::Acquire)?;
        signal.store(value.wrapping_add(1), AtomicWriteOrder::Release)?;
        // Then: both directions preserve the complete little-endian representation.
        assert_eq!(observed, value);
        let mut bytes = [0; 8];
        memory.read(0x1000.into(), MemoryAlignment::ONE, &mut bytes)?;
        assert_eq!(i64::from_le_bytes(bytes), value.wrapping_add(1));
    }
    Ok(())
}

#[test]
fn updates_return_previous_signed_value_and_apply_wrapping_operations() -> TestResult {
    // Given: independent words with signed boundary and bitwise operands.
    for (initial, operand) in [(i64::MAX, 1), (i64::MIN, 1), (-13, 6), (7, -3)] {
        for operation in 0..6 {
            let memory = memory()?;
            memory.write(0x1000.into(), MemoryAlignment::ONE, &initial.to_le_bytes())?;
            let signal = Signal::new(&memory, 0x1000.into());
            // When: a named signal operation updates the existing word.
            let (observed, expected) = match operation {
                0 => (
                    signal.add(operand, AtomicOrder::Relaxed)?,
                    initial.wrapping_add(operand),
                ),
                1 => (
                    signal.subtract(operand, AtomicOrder::Acquire)?,
                    initial.wrapping_sub(operand),
                ),
                2 => (
                    signal.and(operand, AtomicOrder::Release)?,
                    initial & operand,
                ),
                3 => (
                    signal.or(operand, AtomicOrder::AcquireRelease)?,
                    initial | operand,
                ),
                4 => (
                    signal.xor(operand, AtomicOrder::Relaxed)?,
                    initial ^ operand,
                ),
                _ => (
                    signal.exchange(operand, AtomicOrder::AcquireRelease)?,
                    operand,
                ),
            };
            // Then: the return value precedes exactly one update of the full word.
            assert_eq!(observed, initial);
            let mut bytes = [0; 8];
            memory.read(0x1000.into(), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(i64::from_le_bytes(bytes), expected);
        }
    }
    Ok(())
}

#[test]
fn compare_exchange_is_conditional_on_all_signed_bits() -> TestResult {
    for expected in [i64::MIN, i64::MAX] {
        // Given: the same negative word and either a matching or differing expectation.
        let memory = memory()?;
        let initial = i64::MIN;
        let replacement = -17_i64;
        memory.write(0x1000.into(), MemoryAlignment::ONE, &initial.to_le_bytes())?;
        let signal = Signal::new(&memory, 0x1000.into());
        // When: compare/exchange returns the observed value.
        let observed =
            signal.compare_exchange(expected, replacement, AtomicOrder::AcquireRelease)?;
        // Then: only an equal full-width expectation permits replacement.
        assert_eq!(observed, initial);
        assert_eq!(
            signal.load(AtomicReadOrder::Relaxed)?,
            if expected == initial {
                replacement
            } else {
                initial
            }
        );
    }
    Ok(())
}

#[test]
fn competing_additions_preserve_every_update() -> TestResult {
    // Given: several software threads borrow the same supplied memory capability.
    let memory = memory()?;
    let workers = 4_i64;
    let updates = 257_i64;
    // When: all threads repeatedly update the same signal.
    std::thread::scope(|scope| -> TestResult {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let memory = &memory;
                scope.spawn(move || {
                    let signal = Signal::new(memory, 0x1000.into());
                    for _ in 0..updates {
                        signal.add(1, AtomicOrder::Relaxed)?;
                    }
                    Ok::<
                        _,
                        hardware_interface_runtime::AtomicAccessError<
                            hardware_interface_runtime::MemoryAccessError,
                        >,
                    >(())
                })
            })
            .collect();
        for handle in handles {
            handle
                .join()
                .map_err(|_| std::io::Error::other("signal worker panicked"))??;
        }
        Ok(())
    })?;
    // Then: no competing update was lost.
    assert_eq!(
        Signal::new(&memory, 0x1000.into()).load(AtomicReadOrder::Acquire)?,
        workers * updates
    );
    Ok(())
}
