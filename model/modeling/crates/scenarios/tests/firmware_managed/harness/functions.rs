//! Scoped execution of independent Function software programs.
use super::{TestResult, program, runtime::Managed};
use hardware_interface_runtime::{MemoryAddress, SharedMemory, SharedMemoryAllocator};
use std::{sync::Arc, time::Duration};

pub(crate) async fn run_reset() -> TestResult<(
    program::functions::reset::Outcome,
    (pdma_model_contract::Response, Vec<u8>),
)> {
    use program::functions::reset::{self, PeerSignals, ResetSignals};
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 2, 2)?;
    let b = hosts.pop().ok_or("missing Function B")?;
    let a = hosts.pop().ok_or("missing Function A")?;
    let (started, start) = tokio::sync::oneshot::channel();
    let (submitted, submit) = tokio::sync::oneshot::channel();
    let (completed, complete) = tokio::sync::oneshot::channel();
    let first = Box::pin(a.runtime.run_main(async |interrupts| {
        reset::resetting(
            program::Ports {
                bus: a.bus,
                memory: allocator.memory_access(),
                allocator: allocator.clone(),
                interrupts,
            },
            ResetSignals {
                started,
                submitted: submit,
                completed: complete,
            },
        )
        .await
    }));
    let second = Box::pin(b.runtime.run_main(async |interrupts| {
        reset::peer(
            program::Ports {
                bus: b.bus,
                memory: allocator.memory_access(),
                allocator: allocator.clone(),
                interrupts,
            },
            PeerSignals {
                started: start,
                submitted,
                completed,
            },
        )
        .await
    }));
    let (a, b) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async { tokio::join!(first, second) }),
    )
    .await??;
    Ok((a?.0?, b?.0?))
}

/// Runs two IRQ-driven copy programs against one shared managed DMA.
pub async fn run(payloads: [&[u8]; 2]) -> TestResult<[program::functions::Outcome; 2]> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 2, 2)?;
    let b = hosts.pop().ok_or("missing Function B")?;
    let a = hosts.pop().ok_or("missing Function A")?;
    let first = Box::pin(a.runtime.run_main(async |interrupts| {
        program::functions::main(
            program::Ports {
                bus: a.bus,
                memory: allocator.memory_access(),
                allocator: allocator.clone(),
                interrupts,
            },
            payloads[0],
        )
        .await
    }));
    let second = Box::pin(b.runtime.run_main(async |interrupts| {
        program::functions::main(
            program::Ports {
                bus: b.bus,
                memory: allocator.memory_access(),
                allocator: allocator.clone(),
                interrupts,
            },
            payloads[1],
        )
        .await
    }));
    let (a, b) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async { tokio::join!(first, second) }),
    )
    .await??;
    Ok([a?.0?, b?.0?])
}
