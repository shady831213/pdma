//! Platform assembly, actor driving and teardown for single-channel scenarios.

mod fault;

use crate::{hardware, software};
use fault::{FaultBus, UnmappedRing};
use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, Operation};
use hardware_interface_runtime::{
    InterruptRuntime, InterruptSummary, MemoryAddress, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{CommandRegisters, DeviceRegisters, ResponseRegisters};
use pdma_model_host::runtime::Model;
use std::{convert::Infallible, error::Error, sync::Arc};
use tokio::task::JoinSet;

/// Scenario setup or execution infrastructure error.
pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
/// Result of driving the existing hardware pipeline.
pub type WorkerResult = Result<(), AsyncRunError<Infallible, pdma_model_device::Error>>;

/// Test-side fault selection, never passed to the software program.
#[derive(Clone, Copy, Debug)]
pub enum FaultInjection {
    /// Normal memory and transport.
    None,
    /// Reject the response entry write.
    ResponseWrite,
    /// Reject the software command entry write.
    CommandWrite,
    /// Exhaust address space before software allocates its rings.
    AllocationExhausted,
}

/// Observable results retained after execution for case-owned assertions.
#[derive(Debug)]
pub struct Report {
    /// Software completion or its original failure.
    pub program: TestResult<software::Outcome>,
    /// Hardware outcome when a worker ran to completion.
    pub worker: Option<WorkerResult>,
    /// Completed interrupt delivery accounting.
    pub summary: InterruptSummary,
    /// Final hardware interrupt level.
    pub irq: bool,
    /// Backing memory for independent payload and allocation checks.
    pub storage: Arc<SharedMemory>,
}

/// Assemble and drive one complete software program with the requested test fault.
///
/// # Panics
/// Panics if platform setup allocates application buffers before software starts.
pub async fn run(injection: FaultInjection, input: software::Input<'_>) -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let first_address = match injection {
        FaultInjection::AllocationExhausted => u64::MAX,
        FaultInjection::None | FaultInjection::ResponseWrite | FaultInjection::CommandWrite => {
            0x1000
        }
    };
    let allocator =
        SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(first_address));
    let faulted = Arc::new(UnmappedRing::new(allocator.memory_access()));
    let access = SharedMemoryAccess::from(Arc::clone(&faulted));
    let (device, mut stage) = hardware::assemble(access.clone());
    let model = Model::new(device);
    let base_register = match injection {
        FaultInjection::ResponseWrite => Some(u32::try_from(
            DeviceRegisters::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
        )?),
        FaultInjection::CommandWrite => Some(u32::try_from(
            DeviceRegisters::COMMAND.offset() + CommandRegisters::BASE.offset(),
        )?),
        FaultInjection::None | FaultInjection::AllocationExhausted => None,
    };
    let bus = FaultBus {
        bus: model.clone(),
        base_register,
        memory: faulted,
    };
    let cpu = Arc::new(SoftwareCpu::new());
    let mut workers = JoinSet::new();
    match injection {
        FaultInjection::None | FaultInjection::ResponseWrite => {
            let worker_model = model.clone();
            workers.spawn(async move {
                let executor = AsyncExecutor::new(worker_model);
                let mut operation = Operation::new(());
                executor.run(&mut stage, &mut operation).await
            });
        }
        FaultInjection::CommandWrite | FaultInjection::AllocationExhausted => {}
    }
    assert!(
        storage.regions().is_empty(),
        "buffers must be allocated by the running software"
    );
    let runtime = InterruptRuntime::new(cpu, model.clone());
    let (program, summary) = runtime
        .run_main(async |interrupts| {
            software::main(
                software::Ports {
                    bus,
                    memory: access,
                    allocator,
                    interrupts,
                },
                input,
            )
            .await
        })
        .await?;
    let worker = if program.is_ok() {
        workers.join_next().await.transpose()?
    } else {
        None
    };
    drop(workers);
    Ok(Report {
        program,
        worker,
        summary,
        irq: model.irq(),
        storage,
    })
}
