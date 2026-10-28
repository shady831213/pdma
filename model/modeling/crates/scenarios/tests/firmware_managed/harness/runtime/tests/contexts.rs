use super::*;
use pdma_model_contract::memory::{ContextAttributes, FunctionId, MemoryIdentity};

#[test]
fn zero_context_capacity_is_a_context_build_error() {
    // Given: context support with no table entries.
    let memory = SharedMemoryAccess::from(Arc::new(SharedMemory::new()));
    // When: managed hardware construction validates the context capacity.
    let result = Managed::with_context_memory(
        vec![(
            MemoryIdentity {
                function: FunctionId::new(0),
                attributes: ContextAttributes::DEFAULT,
            },
            memory,
        )],
        1,
        super::super::platform::ContextPlatform {
            function_count: 1,
            capacity: 0,
        },
    );
    // Then: context configuration is rejected independently of window setup.
    let error = result.unwrap_err();
    assert!(matches!(
        error,
        super::super::BuildError::Context(
            pdma_model_hw_components::context::ContextConfigError::Layout(
                hw_model_schema::RegisterBankError::ZeroCount
            )
        )
    ));
}

#[tokio::test]
async fn context_aperture_is_firmware_only() {
    // Given: an opt-in context table with an independent platform backing.
    let memory = SharedMemoryAccess::from(Arc::new(SharedMemory::new()));
    let (_system, hosts) = Managed::with_context_memory(
        vec![(
            MemoryIdentity {
                function: FunctionId::new(0),
                attributes: ContextAttributes::DEFAULT,
            },
            memory,
        )],
        1,
        super::super::platform::ContextPlatform {
            function_count: 1,
            capacity: 4,
        },
    )
    .unwrap();
    let host = hosts.first().unwrap();
    let address = host.bus.addresses().contexts.unwrap();
    // When: the Host attempts direct context management MMIO.
    let read = host.bus.read(address, &mut [0; 4]).await;
    let write = host.bus.write(address, &0_u32.to_le_bytes()).await;
    // Then: both transactions are denied by the managed assembly.
    assert!(matches!(read, Err(AccessError::Denied)));
    assert!(matches!(write, Err(AccessError::Denied)));
}

#[test]
fn context_aperture_cannot_overlap_an_existing_hardware_block() {
    // Given: a context table placed at the existing control aperture.
    let addresses = super::super::platform::ADDRESSES;
    let config = pdma_model_firmware::Configuration {
        addresses: pdma_model_software::control::ManagementAddresses {
            contexts: Some(addresses.control),
            ..addresses
        },
        function_count: 1,
    };
    // When: platform assembly validates the generated table extent.
    let result = super::super::platform::validate_functions(config, 1, Some(4));
    // Then: the conflicting aperture is rejected before hardware construction.
    assert!(matches!(
        result,
        Err(super::super::BuildError::Platform(
            super::super::PlatformConfigError::Overlap
        ))
    ));
}
