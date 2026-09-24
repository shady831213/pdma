use super::{
    TestResult, program,
    runtime::{Managed, platform::ContextPlatform},
};
use hardware_interface_runtime::{MemoryAddress, SharedMemory, SharedMemoryAllocator};
use pdma_model_contract::memory::{
    ContextAttributes, ContextAttributesAddressSpace, FunctionId, MemoryIdentity,
};
use std::{sync::Arc, time::Duration};

#[doc = "Independent platform memory attributes used by the context scenarios."]
pub const ATTRIBUTES: [ContextAttributes; 4] = [
    ContextAttributes::DEFAULT,
    ContextAttributes {
        address_space: ContextAttributesAddressSpace::Pasid,
        pasid: 0,
    },
    ContextAttributes {
        address_space: ContextAttributesAddressSpace::Pasid,
        pasid: 1,
    },
    ContextAttributes {
        address_space: ContextAttributesAddressSpace::Pasid,
        pasid: 2,
    },
];

#[doc = "Runs software context management and DMA with optional fault recovery."]
pub async fn run(failure: bool) -> TestResult<program::contexts::Outcome> {
    let allocators: [SharedMemoryAllocator; 4] = std::array::from_fn(|_| {
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000))
    });
    let spaces = ATTRIBUTES
        .iter()
        .zip(&allocators)
        .map(|(attributes, allocator)| {
            (
                MemoryIdentity {
                    function: FunctionId::new(0),
                    attributes: *attributes,
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
            capacity: 4,
        },
    )?;
    let host = hosts.pop().ok_or("missing PF")?;
    let [lists, control, source, destination] = allocators;
    let program = Box::pin(host.runtime.run_main(async |interrupts| {
        program::contexts::main(
            program::Ports {
                bus: host.bus,
                memory: control.memory_access(),
                allocator: control,
                interrupts,
            },
            program::contexts::Spaces {
                lists,
                source,
                destination,
            },
            failure,
        )
        .await
    }));
    let result =
        tokio::time::timeout(Duration::from_secs(5), system.run_until(program)).await???;
    result.0
}
