use super::{Ports, TestResult, setup};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::*;
use pdma_model_host::{CompletionWord, runtime::Model};

pub(in super::super) async fn run(
    ports: Ports<Model<hardware_interface_runtime::SharedMemoryAccess>>,
) -> TestResult {
    let memory = ports.allocator.memory_access();
    let mut channels = setup(&ports).await?;
    let mut peer = channels.pop().ok_or("peer missing")?;
    let mut waiting = channels.pop().ok_or("waiting channel missing")?;
    let storage = ports.allocator.allocate(24, MemoryAlignment::try_new(8)?)?;
    memory.write(storage, MemoryAlignment::ONE, &[0; 24])?;
    let target = |offset| WordTarget {
        address: storage.raw() + offset,
        context: 0,
        width: WordTargetWidth::Bits64,
    };
    let cancelled = CompletionWord(WordWrite {
        target: target(8),
        value: 17,
    });
    let peer_done = CompletionWord(WordWrite {
        target: target(16),
        value: 29,
    });
    waiting
        .submit(Command::wait(
            1,
            WaitCommand {
                target: target(0),
                value: 1,
                comparison: WaitCommandComparison::Equal,
            },
        ))
        .await?;
    waiting.submit(cancelled.command(2)).await?;
    peer.submit(peer_done.command(3)).await?;
    while !peer_done.is_complete(&memory)? {
        assert_eq!(peer.event().await?, None);
        tokio::task::yield_now().await;
    }
    assert!(!waiting.faulted().await?);
    assert!(!cancelled.is_complete(&memory)?);
    waiting.reset().await?;
    memory.write(storage, MemoryAlignment::ONE, &1_u64.to_le_bytes())?;
    let later = CompletionWord(WordWrite {
        target: target(16),
        value: 30,
    });
    peer.submit(later.command(4)).await?;
    while !later.is_complete(&memory)? {
        tokio::task::yield_now().await;
    }
    assert!(!cancelled.is_complete(&memory)?);
    assert_eq!(peer.event().await?, None);
    Ok(())
}
