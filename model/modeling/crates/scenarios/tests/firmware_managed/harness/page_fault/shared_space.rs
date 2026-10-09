use super::{
    Space,
    memory::{PageTable, Paged},
    platform::{Backend, Sources},
};
use crate::harness::{
    TestResult, program,
    runtime::{Managed, platform::ContextPlatform},
};
use hardware_interface_runtime::{SharedMemory, SharedMemoryAllocator};
use pdma_model_contract::memory::{
    ContextAttributes, ContextAttributesAddressSpace, FunctionId, MemoryIdentity,
};
pub use program::page_fault::shared_space::{Lifecycle, Outcome};
use std::{sync::Arc, time::Duration};

pub(in crate::harness) const ATTRIBUTES: ContextAttributes = ContextAttributes {
    address_space: ContextAttributesAddressSpace::Pasid,
    pasid: 7,
};

/// Runs two channels sharing a faulting PASID, then retires handling before channel reuse.
pub async fn run(lifecycle: Lifecycle) -> TestResult<Outcome> {
    let (sender, reports) = tokio::sync::mpsc::channel(4);
    let sources = Sources::new(2);
    let spaces: [Space; 2] = std::array::from_fn(|_| Space {
        allocator: SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into()),
        pages: PageTable::default(),
    });
    let backing = [ContextAttributes::DEFAULT, ATTRIBUTES]
        .into_iter()
        .zip(&spaces)
        .map(|(attributes, space)| {
            let identity = MemoryIdentity {
                function: FunctionId::new(0),
                attributes,
            };
            (
                identity,
                Backend::new(
                    Paged {
                        memory: space.allocator.memory_access(),
                        pages: space.pages.clone(),
                        reports: sender.clone(),
                    },
                    sources.clone(),
                    identity,
                ),
            )
        })
        .collect();
    let (system, mut hosts) = Managed::with_platform_memory(
        (backing, Some(Backend::bind)),
        2,
        ContextPlatform {
            function_count: 1,
            capacity: 2,
        },
    )?;
    let host = hosts.pop().ok_or("missing PF")?;
    let [control, payload] = spaces;
    let execution = Box::pin(host.runtime.run_main(async |interrupts| {
        program::page_fault::shared_space::main(
            program::Ports {
                bus: host.bus,
                memory: control.allocator.memory_access(),
                allocator: control.allocator,
                interrupts,
            },
            program::page_fault::shared_space::Recovery {
                payload,
                sources,
                foreign_sources: Sources::new(2),
                reports,
                lifecycle,
            },
        )
        .await
    }));
    tokio::time::timeout(Duration::from_secs(5), system.run_until(execution))
        .await???
        .0
}
