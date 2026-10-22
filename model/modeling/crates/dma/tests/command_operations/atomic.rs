use super::support::{EVENT_PRODUCER, Rig, TestResult, fence, target};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    AtomicCommand, AtomicCommandOperandsAddValues, AtomicCommandOperandsCompareExchangeValues,
    AtomicCommandOperandsValues, AtomicCommandOrder, Command, SemaphoreCommand,
    SemaphoreCommandOperation,
};

const fn add(id: u64, value: u64) -> Command {
    let operands = AtomicCommandOperandsValues::Add(AtomicCommandOperandsAddValues { value });
    Command::atomic(
        id,
        AtomicCommand {
            target: target(0x8000),
            order: AtomicCommandOrder::AcquireRelease,
            operation: operands.tag(),
            operands,
        },
    )
}

#[test]
fn atomics_commit_once_without_result_storage() -> TestResult {
    // Given: wrapping add and a compare-exchange mismatch.
    let mut rig = Rig::new(1, 3)?;
    rig.memory.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &u64::MAX.to_le_bytes(),
    )?;
    let operands =
        AtomicCommandOperandsValues::CompareExchange(AtomicCommandOperandsCompareExchangeValues {
            compare: 8,
            value: 99,
        });
    let cas = Command::atomic(
        2,
        AtomicCommand {
            target: target(0x8000),
            order: AtomicCommandOrder::Relaxed,
            operation: operands.tag(),
            operands,
        },
    );
    rig.submit(0, &[add(1, 3), cas, fence(0x9000)])?;
    // When: the complete sequence and extra idle steps run.
    rig.drive()?;
    rig.drive()?;
    // Then: addition wraps exactly once and mismatch does not write or report an error.
    assert_eq!(rig.word(0x8000)?, 2);
    assert_eq!(rig.word(0x9000)?, 0xfeed);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn unsupported_atomic_stops_before_mutation_or_fence() -> TestResult {
    // Given: ordinary capabilities reject atomic RMW.
    let mut rig = Rig::new(1, 0)?;
    rig.submit(0, &[add(1, 7), fence(0x9000)])?;
    // When: the unsupported command executes.
    rig.drive()?;
    // Then: one error event stops execution before any target or fence store.
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 1);
    let mut raw = vec![0; pdma_model_contract::dma::Event::RECORD_SIZE];
    rig.memory
        .read(MemoryAddress::new(0x1800), MemoryAlignment::ONE, &mut raw)?;
    assert_eq!(
        pdma_model_contract::dma::Event::from_bytes(&raw)?.code,
        pdma_model_contract::dma::EventCode::Unsupported
    );
    Ok(())
}

#[test]
fn competing_waiters_consume_one_permit_once() -> TestResult {
    // Given: two channels wait on one shared permit.
    let mut rig = Rig::new(2, 3)?;
    rig.memory.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &1_u64.to_le_bytes(),
    )?;
    let wait = Command::semaphore(
        1,
        SemaphoreCommand {
            target: target(0x8000),
            operation: SemaphoreCommandOperation::Wait,
        },
    );
    rig.submit(0, &[wait, fence(0x9000)])?;
    rig.submit(1, &[wait, fence(0x9010)])?;
    // When: both channels repeatedly contend.
    rig.drive()?;
    // Then: precisely one fence completes and the counter cannot underflow.
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(
        usize::from(rig.word(0x9000)? == 0xfeed) + usize::from(rig.word(0x9010)? == 0xfeed),
        1
    );
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert_eq!(rig.register(1, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn semaphore_overflow_stops_without_wrapping() -> TestResult {
    // Given: a saturated permit counter.
    let mut rig = Rig::new(1, 3)?;
    rig.memory.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &u64::MAX.to_le_bytes(),
    )?;
    rig.submit(
        0,
        &[
            Command::semaphore(
                1,
                SemaphoreCommand {
                    target: target(0x8000),
                    operation: SemaphoreCommandOperation::Signal,
                },
            ),
            fence(0x9000),
        ],
    )?;
    // When: Signal attempts to add one permit.
    rig.drive()?;
    // Then: overflow is reported without a counter update or downstream fence.
    assert_eq!(rig.word(0x8000)?, u64::MAX);
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 1);
    Ok(())
}
