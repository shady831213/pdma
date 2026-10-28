//! Scoped execution of independent Function software programs.
use super::{TestResult, program, runtime::Managed};
use hw_model_io::memory::MemoryAddress;
use hw_model_io::memory::{SharedMemory, SharedMemoryAllocator};
use pdma_model_contract::memory::FunctionId;
use program::functions::{Assignment, Grant};
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;

pub(crate) async fn run_reset() -> TestResult<(program::functions::reset::Outcome, (u64, Vec<u8>))>
{
    use program::functions::reset::{self, PeerSignals, ResetSignals};
    let [manager_memory, a_memory, b_memory] = std::array::from_fn(|_| {
        SharedMemoryAllocator::new(
            Arc::new(SharedMemory::new()),
            MemoryAddress::new(0x1000),
            256 * 1024,
        )
    });
    let [manager_memory, a_memory, b_memory] = [manager_memory?, a_memory?, b_memory?];
    let (system, mut hosts) = Managed::with_function_memory(
        vec![
            manager_memory.memory_access(),
            a_memory.memory_access(),
            b_memory.memory_access(),
        ],
        2,
    )?;
    let b = hosts.pop().ok_or("missing Function B")?;
    let a = hosts.pop().ok_or("missing Function A")?;
    let pf = hosts.pop().ok_or("missing PF")?;
    let (assignment_a, grant_a) = assignment(FunctionId::new(1));
    let (assignment_b, grant_b) = assignment(FunctionId::new(2));
    let (reset_request, reset_requested) = oneshot::channel();
    let (reset_done, reset_completed) = oneshot::channel();
    let (started, start) = tokio::sync::oneshot::channel();
    let (submitted, submit) = tokio::sync::oneshot::channel();
    let (completed, complete) = tokio::sync::oneshot::channel();
    let first = Box::pin(a.runtime.run_main(async |interrupts| {
        reset::resetting(
            program::Ports {
                bus: a.bus,
                memory: a_memory.memory_access(),
                allocator: a_memory,
                interrupts,
            },
            grant_a,
            ResetSignals {
                started,
                submitted: submit,
                completed: complete,
                reset: reset_request,
                reset_done: reset_completed,
            },
        )
        .await
    }));
    let second = Box::pin(b.runtime.run_main(async |interrupts| {
        reset::peer(
            program::Ports {
                bus: b.bus,
                memory: b_memory.memory_access(),
                allocator: b_memory,
                interrupts,
            },
            grant_b,
            PeerSignals {
                started: start,
                submitted,
                completed,
            },
        )
        .await
    }));
    let manager = Box::pin(pf.runtime.run_main(async |interrupts| {
        program::functions::manage(
            program::Ports {
                bus: pf.bus,
                memory: manager_memory.memory_access(),
                allocator: manager_memory,
                interrupts,
            },
            [assignment_a, assignment_b],
            Some((reset_requested, reset_done)),
        )
        .await
    }));
    let (result, a, b) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async { tokio::join!(manager, first, second) }),
    )
    .await??;
    result?.0?;
    Ok((a?.0?, b?.0?))
}

/// Runs two IRQ-driven copy programs against one shared managed DMA.
pub async fn run(payloads: [&[u8]; 2]) -> TestResult<[program::functions::Outcome; 2]> {
    let storage = Arc::new(SharedMemory::new());
    let allocator =
        SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000), 256 * 1024)?;
    run_with_memory(payloads, [allocator.clone(), allocator.clone(), allocator]).await
}

/// Gives each Function its own backing at identical DMA addresses.
pub async fn run_isolated(payloads: [&[u8]; 2]) -> TestResult<[program::functions::Outcome; 2]> {
    let [manager, first, second] = std::array::from_fn(|_| {
        SharedMemoryAllocator::new(
            Arc::new(SharedMemory::new()),
            MemoryAddress::new(0x1000),
            256 * 1024,
        )
    });
    run_with_memory(payloads, [manager?, first?, second?]).await
}

async fn run_with_memory(
    payloads: [&[u8]; 2],
    [manager_memory, a_memory, b_memory]: [SharedMemoryAllocator; 3],
) -> TestResult<[program::functions::Outcome; 2]> {
    let (system, mut hosts) = Managed::with_function_memory(
        vec![
            manager_memory.memory_access(),
            a_memory.memory_access(),
            b_memory.memory_access(),
        ],
        2,
    )?;
    let b = hosts.pop().ok_or("missing Function B")?;
    let a = hosts.pop().ok_or("missing Function A")?;
    let pf = hosts.pop().ok_or("missing PF")?;
    let (assignment_a, grant_a) = assignment(FunctionId::new(1));
    let (assignment_b, grant_b) = assignment(FunctionId::new(2));
    let first = Box::pin(a.runtime.run_main(async |interrupts| {
        program::functions::main(
            program::Ports {
                bus: a.bus,
                memory: a_memory.memory_access(),
                allocator: a_memory,
                interrupts,
            },
            payloads[0],
            grant_a,
        )
        .await
    }));
    let second = Box::pin(b.runtime.run_main(async |interrupts| {
        program::functions::main(
            program::Ports {
                bus: b.bus,
                memory: b_memory.memory_access(),
                allocator: b_memory,
                interrupts,
            },
            payloads[1],
            grant_b,
        )
        .await
    }));
    let manager = Box::pin(pf.runtime.run_main(async |interrupts| {
        program::functions::manage(
            program::Ports {
                bus: pf.bus,
                memory: manager_memory.memory_access(),
                allocator: manager_memory,
                interrupts,
            },
            [assignment_a, assignment_b],
            None,
        )
        .await
    }));
    let (result, a, b) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async { tokio::join!(manager, first, second) }),
    )
    .await??;
    result?.0?;
    Ok([a?.0?, b?.0?])
}

fn assignment(function: FunctionId) -> (Assignment, Grant) {
    let (send, channel) = oneshot::channel();
    let (finished, receive) = oneshot::channel();
    (
        Assignment {
            function,
            channel: send,
            finished: receive,
        },
        Grant { channel, finished },
    )
}
