use pdma_model_contract::dma::{
    Command, CommandHeader, CopyCommand, CopyCommandDestinationValues, CopyCommandSourceValues,
    LinearEndpoint, Opcode, TimestampCommand, TrapCommand, WordTarget, WordTargetWidth, WordWrite,
};

use super::{Ports, TestResult, setup};
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use hw_model_io::register::RegisterAccess;
use pdma_model_host::CompletionWord;

pub(in super::super) async fn run<B>(ports: Ports<B>, payload: &[u8; 16]) -> TestResult<[u64; 2]>
where
    B: RegisterAccess + Clone + Send + Sync,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let memory = ports.allocator.memory_access();
    let mut channels = setup(&ports).await?;
    let selected = channels.first_mut().ok_or("channel missing")?;
    let storage = ports.allocator.allocate(64, MemoryAlignment::try_new(8)?)?;
    memory.write(storage, MemoryAlignment::ONE, &[0; 64])?;
    memory.write((storage.raw() + 32).into(), MemoryAlignment::ONE, payload)?;
    let stamp = |offset| TimestampCommand {
        address: storage.raw() + offset,
        context: 0,
        ..TimestampCommand::ZERO
    };
    let completion = CompletionWord(WordWrite {
        target: WordTarget {
            address: storage.raw() + 16,
            context: 0,
            width: WordTargetWidth::Bits64,
        },
        value: 99,
    });
    for command in [
        Command::Timestamp(TimestampCommand {
            header: CommandHeader {
                opcode: Opcode::Timestamp,
            },
            id: 1,
            ..stamp(0)
        }),
        Command::Copy(CopyCommand {
            id: 2,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: storage.raw() + 32,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: storage.raw() + 48,
                offset: 0,
            }),
            copy_length: 16,
            sg_context: 0,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
        Command::Timestamp(TimestampCommand {
            header: CommandHeader {
                opcode: Opcode::Timestamp,
            },
            id: 3,
            ..stamp(8)
        }),
        completion.command(4),
        Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 5,
        }),
    ] {
        selected.submit(command).await?;
    }
    loop {
        if let Some(event) = selected.event().await? {
            return Err(format!("timestamp batch stopped: {event:?}").into());
        }
        if selected.irq_cause().await?.trap() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(completion.is_complete(&memory)?);
    let mut copied = [0; 16];
    memory.read(
        (storage.raw() + 48).into(),
        MemoryAlignment::ONE,
        &mut copied,
    )?;
    assert_eq!(&copied, payload);
    let mut stamps = [0; 16];
    memory.read(storage, MemoryAlignment::ONE, &mut stamps)?;
    let start = u64::from_le_bytes(stamps[..8].try_into()?);
    let end = u64::from_le_bytes(stamps[8..].try_into()?);
    selected.acknowledge_trap().await?;
    Ok([start, end])
}
