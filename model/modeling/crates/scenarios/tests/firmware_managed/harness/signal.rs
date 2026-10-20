use super::{
    TestResult, program,
    runtime::{Managed, platform::ContextPlatform},
};
use hardware_interface_runtime::{MemoryAddress, SharedMemory, SharedMemoryAllocator};
use pdma_model_contract::memory::{ContextAttributes, FunctionId, MemoryIdentity};
use program::signal::{Input, Outcome, PASID_ZERO};
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;

/// Runs pending-Drain recovery and fresh work through firmware-managed channels.
pub async fn run_recovery(input: Input) -> TestResult<program::signal::recovery::Outcome> {
    let allocators: [SharedMemoryAllocator; 2] = std::array::from_fn(|_| {
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000))
    });
    let spaces = [ContextAttributes::DEFAULT, PASID_ZERO]
        .into_iter()
        .zip(&allocators)
        .map(|(attributes, allocator)| {
            (
                MemoryIdentity {
                    function: FunctionId::new(0),
                    attributes,
                },
                allocator.memory_access(),
            )
        })
        .collect();
    let (system, mut hosts) = Managed::with_context_memory(
        spaces,
        1,
        ContextPlatform {
            function_count: 1,
            capacity: 2,
        },
    )?;
    let host = hosts.pop().ok_or("missing PF")?;
    let [default, pasid] = allocators;
    let program = Box::pin(host.runtime.run_main(async |interrupts| {
        program::signal::recovery::run(
            program::Ports {
                bus: host.bus,
                memory: default.memory_access(),
                allocator: default,
                interrupts,
            },
            pasid,
            input,
        )
        .await
    }));
    tokio::time::timeout(Duration::from_secs(5), system.run_until(program))
        .await???
        .0
}

/// Runs peer Signal/Wait ordering across isolated Function and PASID backings.
pub async fn run(input: Input) -> TestResult<([i64; 2], Outcome)> {
    let allocators: [SharedMemoryAllocator; 4] = std::array::from_fn(|_| {
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000))
    });
    let identities = [
        (0, ContextAttributes::DEFAULT),
        (0, PASID_ZERO),
        (1, ContextAttributes::DEFAULT),
        (1, PASID_ZERO),
    ];
    let spaces = identities
        .into_iter()
        .zip(&allocators)
        .map(|((function, attributes), allocator)| {
            (
                MemoryIdentity {
                    function: FunctionId::new(function),
                    attributes,
                },
                allocator.memory_access(),
            )
        })
        .collect();
    let (system, mut hosts) = Managed::with_context_memory(
        spaces,
        2,
        ContextPlatform {
            function_count: 2,
            capacity: 2,
        },
    )?;
    let vf = hosts.pop().ok_or("missing VF")?;
    let pf = hosts.pop().ok_or("missing PF")?;
    let [pf_default, pf_pasid, vf_default, vf_pasid] = allocators;
    let (send_grant, grant) = oneshot::channel();
    let (finished, receive_finished) = oneshot::channel();
    let manager = Box::pin(pf.runtime.run_main(async |interrupts| {
        program::signal::manage(
            program::Ports {
                bus: pf.bus,
                memory: pf_default.memory_access(),
                allocator: pf_default,
                interrupts,
            },
            pf_pasid,
            input,
            send_grant,
            receive_finished,
        )
        .await
    }));
    let worker = Box::pin(vf.runtime.run_main(async |interrupts| {
        program::signal::run(
            program::Ports {
                bus: vf.bus,
                memory: vf_default.memory_access(),
                allocator: vf_default,
                interrupts,
            },
            vf_pasid,
            input,
            grant,
            finished,
        )
        .await
    }));
    let (manager, worker) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async { tokio::join!(manager, worker) }),
    )
    .await??;
    Ok((manager?.0?, worker?.0?))
}
