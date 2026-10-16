//! Backing selection, actor execution and joined teardown belong to the harness.

use hardware_interface_runtime::{
    InterruptRuntime, InterruptSummary, MemoryAddress, SharedMemory, SharedMemoryAllocator,
    SoftwareCpu,
};
use std::{io, sync::Arc};
use tokio::task::JoinSet;

use crate::{Scenario, TestResult, hardware, software};

pub(super) struct Report {
    pub(super) outcome: software::Outcome,
    pub(super) summary: InterruptSummary,
    pub(super) storage: Arc<SharedMemory>,
    pub(super) pending: bool,
}

pub(super) async fn run(scenario: Scenario) -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (model, hardware) = hardware::assemble(memory.clone())?;
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
    let mut workers = JoinSet::new();
    workers.spawn(hardware::drive(model.clone(), hardware));
    assert!(storage.regions().is_empty());
    let execution = runtime.run_main(async |interrupts| {
        software::main(
            software::Ports {
                bus: model.clone(),
                memory,
                allocator,
                interrupts,
            },
            scenario,
        )
        .await
    });
    let result: TestResult<_> = tokio::select! {
        biased;
        result = execution => result.map_err(Into::into),
        () = async { for _ in 0..50_000 { tokio::task::yield_now().await; } } => {
            Err(io::Error::other("multi-channel scenario exhausted its scheduling budget").into())
        }
    };
    workers.abort_all();
    while let Some(joined) = workers.join_next().await {
        match joined {
            Ok(result) => result?,
            Err(error) if error.is_cancelled() => {}
            Err(error) => return Err(error.into()),
        }
    }
    let (outcome, summary) = result?;
    Ok(Report {
        outcome: outcome?,
        summary,
        storage,
        pending: model.irq(),
    })
}
