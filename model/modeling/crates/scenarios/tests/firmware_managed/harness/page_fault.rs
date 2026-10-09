mod memory;
pub(in crate::harness) mod platform;
/// Shared-address-space recovery assembly.
pub mod shared_space;

use super::{
    TestResult,
    contexts::ATTRIBUTES,
    program,
    runtime::{Managed, platform::ContextPlatform},
};
use hardware_interface_runtime::{MemoryAddress, SharedMemory, SharedMemoryAllocator};
pub(in crate::harness) use memory::PageTable;
pub use memory::{FaultAccess, FaultReport};
use pdma_model_contract::memory::{FunctionId, MemoryIdentity};
pub use platform::FaultTag;
use std::{sync::Arc, time::Duration};

pub(in crate::harness) struct Space {
    pub allocator: SharedMemoryAllocator,
    pub pages: PageTable,
}

/// Runs isolated address spaces and software repair against the managed hardware.
pub async fn run() -> TestResult<(
    program::page_fault::Outcome,
    Result<(), super::runtime::AccessError>,
)> {
    let (reports, faults) = tokio::sync::mpsc::channel(2);
    let sources = platform::Sources::new(1);
    let spaces: [Space; 4] = std::array::from_fn(|_| Space {
        allocator: SharedMemoryAllocator::new(
            Arc::new(SharedMemory::new()),
            MemoryAddress::new(0x1000),
        ),
        pages: PageTable::default(),
    });
    let mut backing = backing(&spaces, &sources, &reports);
    let management =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let identity = MemoryIdentity {
        function: FunctionId::new(0),
        attributes: ATTRIBUTES[1],
    };
    backing.push((
        identity,
        platform::Backend::new(
            memory::Paged {
                memory: management.memory_access(),
                pages: PageTable::default(),
                reports,
            },
            sources.clone(),
            identity,
        ),
    ));
    let (system, mut hosts) = Managed::with_platform_memory(
        (backing, Some(platform::Backend::bind)),
        1,
        ContextPlatform {
            function_count: 2,
            capacity: 4,
        },
    )?;
    let host = hosts.pop().ok_or("VF missing")?;
    let pf = hosts.pop().ok_or("PF missing")?;
    let [lists, control, source, destination] = spaces;
    let (grant, granted) = tokio::sync::oneshot::channel();
    let (finished, done) = tokio::sync::oneshot::channel();
    let (released, release) = tokio::sync::oneshot::channel();
    let execution = Box::pin(host.runtime.run_main(async |interrupts| {
        program::page_fault::main(
            program::Ports {
                bus: host.bus,
                memory: control.allocator.memory_access(),
                allocator: control.allocator,
                interrupts,
            },
            program::page_fault::Recovery {
                sources,
                spaces: [lists, source, destination],
                reports: faults,
                grant: granted,
                finished,
                released: release,
            },
        )
        .await
    }));
    let manager = Box::pin(pf.runtime.run_main(async |interrupts| {
        program::page_fault::management::run(
            program::Ports {
                bus: pf.bus,
                memory: management.memory_access(),
                allocator: management,
                interrupts,
            },
            program::page_fault::management::Lifecycle {
                grant,
                finished: done,
                released,
            },
        )
        .await
    }));
    let (manager, result) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async { tokio::join!(manager, execution) }),
    )
    .await??;
    Ok((result?.0?, manager?.0?))
}

fn backing(
    spaces: &[Space; 4],
    sources: &platform::Sources,
    reports: &tokio::sync::mpsc::Sender<FaultReport>,
) -> Vec<(MemoryIdentity, platform::Backend)> {
    ATTRIBUTES
        .into_iter()
        .zip(spaces)
        .map(|(attributes, space)| {
            (
                MemoryIdentity {
                    function: FunctionId::new(1),
                    attributes,
                },
                platform::Backend::new(
                    memory::Paged {
                        memory: space.allocator.memory_access(),
                        pages: space.pages.clone(),
                        reports: reports.clone(),
                    },
                    sources.clone(),
                    MemoryIdentity {
                        function: FunctionId::new(1),
                        attributes,
                    },
                ),
            )
        })
        .collect()
}
