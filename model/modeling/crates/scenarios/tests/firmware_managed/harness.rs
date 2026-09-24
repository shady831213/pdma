//! Scoped platform assembly and scheduling for managed system cases.

#[doc = "Context-capable scenario platform assembly."]
pub mod contexts;
pub mod functions;
pub mod multi;
pub mod program;
pub mod runtime;

use hardware_interface_runtime::{
    InterruptSummary, MemoryAddress, SharedMemory, SharedMemoryAllocator,
};
use runtime::Managed;
use std::{error::Error, io, sync::Arc};

pub use program::Input;
/// Scenario setup or execution failure.
pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

/// Case-visible results retained after all scoped actors stop.
#[derive(Debug)]
pub struct Report {
    pub(super) program: TestResult<program::Outcome>,
    pub(super) summary: InterruptSummary,
    pub(super) storage: Arc<SharedMemory>,
}

/// Drives one Host program with production hardware and firmware within a scheduling budget.
pub async fn run(input: Input<'_>) -> TestResult<Report> {
    run_at(input, runtime::platform::ADDRESSES).await
}

/// Runs the same software programs with an alternate platform register placement.
pub async fn run_at(
    input: Input<'_>,
    addresses: pdma_model_software::control::ManagementAddresses,
) -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::with_addresses(memory.clone(), 1, addresses)?;
    let execution = Box::pin(host.runtime.run_main(async |interrupts| {
        program::main(
            program::Ports {
                bus: host.bus,
                memory,
                allocator,
                interrupts,
            },
            input,
        )
        .await
    }));
    let hardware = async {
        tokio::task::yield_now().await;
        system
            .run()
            .await
            .map_err(Into::<Box<dyn Error + Send + Sync>>::into)
    };
    let budget = async {
        for _ in 0..10_000 {
            tokio::task::yield_now().await;
        }
    };
    let (program, summary) = tokio::select! {
        biased;
        result = hardware => match result {
            Ok(never) => match never {},
            Err(error) => return Err(error),
        },
        result = execution => result?,
        () = budget => return Err(io::Error::other("managed scenario scheduling budget exceeded").into()),
    };
    Ok(Report {
        program,
        summary,
        storage,
    })
}
