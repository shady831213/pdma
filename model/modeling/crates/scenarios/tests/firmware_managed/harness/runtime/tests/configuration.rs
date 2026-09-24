use super::super::{BuildError, PlatformConfigError, platform};
use super::Managed;
use hardware_interface_runtime::SharedMemory;
use pdma_model_firmware::Configuration;
use std::error::Error;

#[test]
fn invalid_register_population_preserves_the_schema_cause() {
    // Given: no channels can be represented by the required register banks.
    // When: the platform validates its hardware population.
    let error = Managed::with_channels(SharedMemory::new(), 0).unwrap_err();
    // Then: both the platform classification and original schema cause are available.
    assert!(matches!(
        error,
        BuildError::Platform(PlatformConfigError::Layout(
            hardware_interface_schema::RegisterBankError::ZeroCount
        ))
    ));
    let cause = error.source().unwrap().source().unwrap();
    assert_eq!(
        cause.downcast_ref::<hardware_interface_schema::RegisterBankError>(),
        Some(&hardware_interface_schema::RegisterBankError::ZeroCount)
    );
}

#[test]
fn invalid_function_namespace_is_not_a_window_failure() {
    // Given: an empty or unrepresentable Function namespace, with one valid DMA channel.
    let counts = std::iter::once(0).chain(usize::try_from(u64::from(u32::MAX) + 1));
    for count in counts {
        // When: the platform is assembled before any actors start.
        let error = Managed::with_functions(SharedMemory::new(), 1, count).unwrap_err();
        // Then: namespace rejection belongs to platform configuration, not channel windows.
        assert!(
            matches!(
                error,
                BuildError::Platform(PlatformConfigError::FunctionCount)
            ),
            "{error:?}"
        );
    }
}

#[test]
fn overlapping_control_and_firmware_blocks_are_not_a_window_failure() {
    // Given: two peripheral blocks overlap while the DMA and window apertures are valid.
    let mut addresses = platform::ADDRESSES;
    addresses.firmware = addresses.control;
    // When: platform placement is validated.
    let error = Managed::with_addresses(SharedMemory::new(), 1, addresses).unwrap_err();
    // Then: the collision is not attributed to a hardware window.
    assert!(
        matches!(error, BuildError::Platform(PlatformConfigError::Overlap)),
        "{error:?}"
    );
}

#[test]
fn invalid_control_placement_is_not_a_window_failure() {
    // Given: a misaligned base or an aligned block extending beyond the 32-bit bus.
    for base in [platform::ADDRESSES.control + 1, !7_u32] {
        let mut addresses = platform::ADDRESSES;
        addresses.control = base;
        // When: the platform is assembled.
        let error = Managed::with_addresses(SharedMemory::new(), 1, addresses).unwrap_err();
        // Then: platform address validation reports its own error.
        assert!(
            matches!(
                error,
                BuildError::Platform(PlatformConfigError::AddressSpace)
            ),
            "{error:?}"
        );
    }
}

#[test]
fn incomplete_context_configuration_is_not_a_window_failure() {
    // Given: context capacity without a matching MMIO base, or the reverse.
    for (base, capacity) in [(None, Some(2)), (Some(0x0013_0000), None)] {
        let mut addresses = platform::ADDRESSES;
        addresses.contexts = base;
        // When: the platform validates its optional context configuration.
        let error = platform::validate_functions(
            Configuration {
                addresses,
                function_count: 1,
            },
            1,
            capacity,
        )
        .unwrap_err();
        // Then: an incomplete platform setting is not a window geometry failure.
        assert!(
            matches!(
                error,
                BuildError::Platform(PlatformConfigError::ContextConfiguration)
            ),
            "{error:?}"
        );
    }
}

#[test]
fn disjoint_relocated_platform_constructs_all_function_ports() {
    // Given: valid disjoint register placements and three Function ports.
    let addresses = pdma_model_software::control::ManagementAddresses {
        dma: 0x2000,
        windows: 0x0020_0000,
        control: 0x0021_0000,
        firmware: 0x0022_0000,
        contexts: None,
    };
    // When: ordinary and relocated assemblies are constructed.
    let (_, hosts) = Managed::with_functions(SharedMemory::new(), 2, 3).unwrap();
    let (_, host) = Managed::with_addresses(SharedMemory::new(), 2, addresses).unwrap();
    // Then: port population and platform placement survive validation.
    assert_eq!(hosts.len(), 3);
    assert_eq!(host.bus.addresses().dma, addresses.dma);
    assert_eq!(host.bus.addresses().control, addresses.control);
}
