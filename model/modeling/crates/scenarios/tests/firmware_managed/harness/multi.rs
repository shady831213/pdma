//! Scoped assembly for the three-channel lifecycle scenario.

use super::{TestResult, program, runtime::Managed};
use hardware_interface_io::memory::MemoryAddress;
use hardware_interface_io::memory::{SharedMemory, SharedMemoryAllocator};
use hardware_interface_software::InterruptSummary;
use std::{io, sync::Arc};

#[derive(Debug)]
/// Software observations retained after all runtime owners have stopped.
pub struct Report {
    pub(crate) program: TestResult<program::multi::Outcome>,
    pub(crate) summary: InterruptSummary,
    pub(crate) storage: Arc<SharedMemory>,
}

/// Drives the real managed hardware and Host program with a bounded scheduling budget.
pub async fn run() -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::with_channels(memory.clone(), 3)?;
    let execution = Box::pin(host.runtime.run_main(async |interrupts| {
        program::multi::main(program::Ports {
            bus: host.bus,
            memory,
            allocator,
            interrupts,
        })
        .await
    }));
    let budget = async {
        for _ in 0..20_000 {
            tokio::task::yield_now().await;
        }
    };
    let (program, summary) = tokio::select! {
        biased;
        result = system.run_until(execution) => result??,
        () = budget => return Err(io::Error::other("multi-channel lifecycle scheduling budget exceeded").into()),
    };
    Ok(Report {
        program,
        summary,
        storage,
    })
}
