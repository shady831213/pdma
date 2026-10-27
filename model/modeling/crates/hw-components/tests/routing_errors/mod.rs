mod backend;
mod context;

use backend::{ADDRESS, BackendError, PagedMemory};
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{
    dma::ChannelId,
    memory::{ContextAttributes, FunctionId, MemoryIdentity},
};
use pdma_model_dma::MemoryFailure;
use pdma_model_hw_components::{
    control::{ControlChannel, MemoryError as ControlError},
    windows::{ChannelWindows, MemoryError as WindowError},
};

#[test]
fn window_preserves_missing_page_and_rejects_unowned_channel_before_backend() {
    // Given: a Function-zero window and real storage behind a missing-page backend.
    let memory = PagedMemory::new(FunctionId::new(0)).unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let port = windows.memory(&memory);
    let mut bytes = [99; 4];
    // When: an authorized write faults and an unowned channel tries all access methods.
    let error = port
        .write_with_sideband(&ChannelId::new(0), ADDRESS, MemoryAlignment::ONE, &[44; 4])
        .unwrap_err();
    assert_eq!(
        error,
        WindowError::Downstream(BackendError::MissingPage { address: ADDRESS })
    );
    assert_eq!(
        std::error::Error::source(&error).unwrap().to_string(),
        BackendError::MissingPage { address: ADDRESS }.to_string()
    );
    assert_eq!(
        MemoryFailure::from(error),
        MemoryFailure::PageFault { address: ADDRESS }
    );
    memory.present.set(true);
    let channel = ChannelId::new(1);
    let rejected = [
        port.validate_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, 4),
        port.read_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &mut bytes),
        port.write_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &[55; 4]),
    ];
    // Then: hardware rejection is Access, and neither rejected path reaches backing storage.
    for result in rejected {
        let error = result.unwrap_err();
        assert_eq!(error, WindowError::UnownedChannel { channel });
        assert_eq!(MemoryFailure::from(error), MemoryFailure::Access);
    }
    assert_eq!(memory.calls.get(), 1);
    assert_eq!(bytes, [99; 4]);
    assert_eq!(
        memory
            .backing
            .read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [13; 4]
    );
}

#[test]
fn control_preserves_custom_errors_and_uses_its_fixed_manager_after_repair() {
    // Given: independent control memory bound to manager seven.
    let manager = FunctionId::new(7);
    let memory = PagedMemory::new(MemoryIdentity {
        function: manager,
        attributes: ContextAttributes::DEFAULT,
    })
    .unwrap();
    let control = ControlChannel::new(0, manager);
    let port = control.memory(&memory);
    let mut bytes = [99; 4];
    // When: validation, read and write meet a missing page.
    let failures = [
        port.validate(ADDRESS, MemoryAlignment::ONE, 4),
        port.read(ADDRESS, MemoryAlignment::ONE, &mut bytes),
        port.write(ADDRESS, MemoryAlignment::ONE, &[44; 4]),
    ];
    // Then: each operation preserves the typed cause without committing data; repair permits real I/O.
    for result in failures {
        let error = result.unwrap_err();
        assert_eq!(
            error,
            ControlError::Downstream(BackendError::MissingPage { address: ADDRESS })
        );
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            BackendError::MissingPage { address: ADDRESS }.to_string()
        );
    }
    assert_eq!(bytes, [99; 4]);
    assert_eq!(
        memory
            .backing
            .read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [13; 4]
    );
    memory.present.set(true);
    port.write(ADDRESS, MemoryAlignment::ONE, &[44; 4]).unwrap();
    assert_eq!(
        port.read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [44; 4]
    );
}
