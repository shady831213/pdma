use super::{Ports, TestResult, setup};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, RegisterAccess};
use pdma_model_contract::dma::{
    Command, CopyCommand, Endpoint, TimestampCommand, WordTarget, WordTargetWidth, WordWrite,
};
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
        Command::timestamp(1, stamp(0)),
        Command::copy(
            2,
            CopyCommand {
                source: Endpoint::linear(storage.raw() + 32, 0),
                destination: Endpoint::linear(storage.raw() + 48, 0),
                copy_length: 16,
                sg_context: 0,
            },
        ),
        Command::timestamp(3, stamp(8)),
        completion.command(4),
        Command::trap(5),
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
