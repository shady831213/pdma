use pdma_model_contract::dma::*;

pub(super) fn batch(address: u64, semaphore_first: bool) -> [Command; 9] {
    let target = |offset, width| WordTarget {
        address: address + offset,
        context: 0,
        width,
    };
    let word = target(8, WordTargetWidth::Bits32);
    let permits = target(16, WordTargetWidth::Bits64);
    let atomic = if semaphore_first {
        Command::semaphore(
            5,
            SemaphoreCommand {
                target: permits,
                operation: SemaphoreCommandOperation::Wait,
            },
        )
    } else {
        Command::atomic(
            5,
            AtomicCommand {
                target: word,
                order: AtomicCommandOrder::AcquireRelease,
                operation: AtomicCommandOperation::Add,
                operands: AtomicCommandOperandsValues::Add(AtomicCommandOperandsAddValues {
                    value: 5,
                }),
            },
        )
    };
    [
        Command::wait(
            1,
            WaitCommand {
                target: target(0, WordTargetWidth::Bits64),
                value: 1,
                comparison: WaitCommandComparison::Equal,
            },
        ),
        Command::write(
            2,
            WordWrite {
                target: word,
                value: 7,
            },
        ),
        Command::fill(
            3,
            FillCommand {
                address: address + 33,
                length: 11,
                pattern: 0x4433_2211,
                context: 0,
                width: FillCommandWidth::Bits32,
            },
        ),
        Command::write(
            4,
            WordWrite {
                target: permits,
                value: 1,
            },
        ),
        atomic,
        Command::semaphore(
            6,
            SemaphoreCommand {
                target: permits,
                operation: SemaphoreCommandOperation::Wait,
            },
        ),
        Command::semaphore(
            7,
            SemaphoreCommand {
                target: permits,
                operation: SemaphoreCommandOperation::Signal,
            },
        ),
        Command::fence(
            8,
            WordWrite {
                target: target(24, WordTargetWidth::Bits64),
                value: 99,
            },
        ),
        Command::trap(9),
    ]
}
