//! Hardware assembly, sustained execution and joined teardown for recovery cases.

use super::{Scenario, software};
use crate::harness::{TestResult, lifecycle};
use hardware_interface_io::memory::MemoryAddress;
use hardware_interface_io::memory::{SharedMemory, SharedMemoryAllocator};
use hardware_interface_software::{InterruptRuntime, InterruptSummary, SoftwareEnv};
use pdma_model_host::runtime::Model;
use std::sync::Arc;
use tokio::task::JoinError;

pub(super) struct Report {
    pub(super) outcome: software::Outcome,
    pub(super) summary: InterruptSummary,
    pub(super) storage: Arc<SharedMemory>,
    pub(super) irq: bool,
    pub(super) worker: JoinError,
}

pub(super) async fn run(scenario: Scenario, payload: &[u8]) -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (device, hardware) = crate::hardware::assemble(memory.clone())?;
    let model = Model::new(device);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareEnv::new()), model.clone());
    assert!(storage.regions()?.is_empty());
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
    let ((outcome, summary), worker) = lifecycle::run(
        crate::hardware::drive(model.clone(), hardware),
        Box::pin(async { execution.await.map_err(Into::into) }),
    )
    .await?;
    Ok(Report {
        outcome: outcome?,
        summary,
        storage,
        irq: model.irq(),
        worker,
    })
}
