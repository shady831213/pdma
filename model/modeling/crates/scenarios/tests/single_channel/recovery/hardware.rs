//! Hardware assembly, sustained execution and joined teardown for recovery cases.

use super::{Scenario, software};
use crate::harness::TestResult;
use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, Operation,
};
use hardware_interface_runtime::{
    InterruptRuntime, InterruptSummary, MemoryAddress, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_device::{Device, Error, Pipeline};
use pdma_model_host::runtime::Model;
use std::{io, sync::Arc};
use tokio::task::JoinSet;

pub(super) struct Report {
    pub(super) outcome: software::Outcome,
    pub(super) summary: InterruptSummary,
    pub(super) storage: Arc<SharedMemory>,
    pub(super) irq: bool,
}

async fn drive(model: Model<SharedMemoryAccess>, mut stage: Pipeline) -> TestResult {
    let executor = AsyncExecutor::new(model.clone());
    loop {
        let mut operation = Operation::new(());
        match executor.run(&mut stage, &mut operation).await {
            Ok(()) | Err(AsyncRunError::Stage(Error::Reset)) => {}
            Err(AsyncRunError::Stage(Error::Halted)) => model.wait(()).await?,
            Err(AsyncRunError::Stage(error)) => return Err(error.into()),
            Err(AsyncRunError::Exhausted) => {
                return Err(io::Error::other("exhausted fresh operation").into());
            }
            Err(AsyncRunError::BackendInteraction(error) | AsyncRunError::BackendWait(error)) => {
                match error {}
            }
        }
    }
}

pub(super) async fn run(scenario: Scenario, payload: &[u8]) -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let device = Device::new(memory.clone(), 1)?;
    let stage = device.pipeline();
    let model = Model::new(device);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
    let mut workers = JoinSet::new();
    workers.spawn(drive(model.clone(), stage));
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
            payload,
        )
        .await
    });
    let result: TestResult<_> = tokio::select! {
        biased;
        result = execution => result.map_err(Into::into),
        () = async { for _ in 0..10_000 { tokio::task::yield_now().await; } } => {
            Err(io::Error::other("recovery scenario exhausted its scheduling budget").into())
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
        irq: model.irq(),
    })
}
