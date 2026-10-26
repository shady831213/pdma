use pdma_model_contract::dma::*;
use pdma_model_contract::dma::{
    AtomicCommand, CommandHeader, FenceCommand, FillCommand, Opcode, SemaphoreCommand, TrapCommand,
    WaitCommand, WriteCommand,
};

pub(super) fn batch(address: u64, semaphore_first: bool) -> [Command; 9] {
    let target = |offset, width| WordTarget {
        address: address + offset,
        context: 0,
        width,
    };
    let word = target(8, WordTargetWidth::Bits32);
    let permits = target(16, WordTargetWidth::Bits64);
    let atomic = operation(word, permits, semaphore_first);
    [
        Command::Wait(WaitCommand {
            header: CommandHeader {
                opcode: Opcode::Wait,
            },
            id: 1,
            target: target(0, WordTargetWidth::Bits64),
            value: 1,
            comparison: WaitCommandComparison::Equal,
        }),
        Command::Write(WriteCommand {
            header: CommandHeader {
                opcode: Opcode::Write,
            },
            id: 2,
            word: WordWrite {
                target: word,
                value: 7,
            },
        }),
        Command::Fill(FillCommand {
            header: CommandHeader {
                opcode: Opcode::Fill,
            },
            id: 3,
            address: address + 33,
            length: 11,
            pattern: 0x4433_2211,
            context: 0,
            width: FillCommandWidth::Bits32,
        }),
        Command::Write(WriteCommand {
            header: CommandHeader {
                opcode: Opcode::Write,
            },
            id: 4,
            word: WordWrite {
                target: permits,
                value: 1,
            },
        }),
        atomic,
        Command::Semaphore(SemaphoreCommand {
            header: CommandHeader {
                opcode: Opcode::Semaphore,
            },
            id: 6,
            target: permits,
            operation: SemaphoreCommandOperation::Wait,
        }),
        Command::Semaphore(SemaphoreCommand {
            header: CommandHeader {
                opcode: Opcode::Semaphore,
            },
            id: 7,
            target: permits,
            operation: SemaphoreCommandOperation::Signal,
        }),
        Command::Fence(FenceCommand {
            header: CommandHeader {
                opcode: Opcode::Fence,
            },
            id: 8,
            word: WordWrite {
                target: target(24, WordTargetWidth::Bits64),
                value: 99,
            },
        }),
        Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 9,
        }),
    ]
}

fn operation(word: WordTarget, permits: WordTarget, semaphore_first: bool) -> Command {
    if semaphore_first {
        Command::Semaphore(SemaphoreCommand {
            header: CommandHeader {
                opcode: Opcode::Semaphore,
            },
            id: 5,
            target: permits,
            operation: SemaphoreCommandOperation::Wait,
        })
    } else {
        Command::Atomic(AtomicCommand {
            header: CommandHeader {
                opcode: Opcode::Atomic,
            },
            id: 5,
            target: word,
            order: AtomicCommandOrder::AcquireRelease,
            operation: AtomicCommandOperation::Add,
            operands: AtomicCommandOperandsValues::Add(AtomicCommandOperandsAddValues { value: 5 }),
        })
    }
}
