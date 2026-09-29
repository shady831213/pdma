mod memory;

use super::{
    TestResult,
    contexts::ATTRIBUTES,
    program,
    runtime::{Managed, platform::ContextPlatform},
};
use hardware_interface_runtime::{
    MemoryAddress, SharedMemory, SharedMemoryAccess, SharedMemoryAllocator,
};
pub(in crate::harness) use memory::PageTable;
use pdma_model_contract::memory::{FunctionId, MemoryIdentity};
use std::{sync::Arc, time::Duration};

pub(in crate::harness) struct Space {
    pub allocator: SharedMemoryAllocator,
    pub pages: PageTable,
}

/// Runs isolated address spaces and software repair against the managed hardware.
pub async fn run() -> TestResult<program::page_fault::Outcome> {
    let spaces: [Space; 4] = std::array::from_fn(|_| Space {
        allocator: SharedMemoryAllocator::new(
            Arc::new(SharedMemory::new()),
            MemoryAddress::new(0x1000),
        ),
        pages: PageTable::default(),
    });
    let backing = ATTRIBUTES
        .into_iter()
        .zip(&spaces)
        .map(|(attributes, space)| {
            (
                MemoryIdentity {
                    function: FunctionId::new(0),
                    attributes,
                },
                SharedMemoryAccess::from(Arc::new(memory::Paged {
                    memory: space.allocator.memory_access(),
                    pages: space.pages.clone(),
                })),
            )
        })
        .collect();
    let (system, mut hosts) = Managed::with_context_memory(
        backing,
        1,
        ContextPlatform {
            function_count: 1,
            capacity: 4,
        },
    )?;
    let host = hosts.pop().ok_or("PF missing")?;
    let [lists, control, source, destination] = spaces;
    let execution = Box::pin(host.runtime.run_main(async |interrupts| {
        program::page_fault::main(
            program::Ports {
                bus: host.bus,
                memory: control.allocator.memory_access(),
                allocator: control.allocator,
                interrupts,
            },
            [lists, source, destination],
        )
        .await
    }));
    let result =
        tokio::time::timeout(Duration::from_secs(5), system.run_until(execution)).await???;
    result.0
}
