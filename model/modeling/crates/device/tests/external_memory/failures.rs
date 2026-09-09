use super::*;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::GlobalRegisters;
use pdma_model_device::DeviceCore;

#[test]
fn runtime_construction_keeps_the_core_external_backend() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a core attached to externally owned sentinel bytes.
    let external = Arc::new(ExternalBytes(Mutex::new([7; 128])));
    let core = DeviceCore::new(external.clone(), ResourceConfig::default());

    // When: the runtime is constructed directly from that core.
    let (device, _worker) = AsyncDevice::from_core(core);
    device.handle().memory_access().write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &[1, 2, 3, 4],
    )?;

    // Then: runtime memory access reaches the same external byte store.
    let mut bytes = [0; 4];
    external.read(MemoryAddress::new(0x8000), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [1, 2, 3, 4]);
    Ok(())
}

#[test]
fn external_access_preserves_destination_when_extent_is_unavailable() {
    // Given a real fixed external storage backend with sentinel bytes.
    let external = ExternalBytes(Mutex::new([7; 128]));
    let mut destination = [4; 4];

    // When accesses extend beyond its existing storage.
    let read = external.read(
        MemoryAddress::new(0x807f),
        MemoryAlignment::ONE,
        &mut destination,
    );
    let write = external.write(MemoryAddress::new(0x807f), MemoryAlignment::ONE, &[9, 8]);

    // Then failed accesses preserve the caller's output and external bytes.
    assert!(read.is_err());
    assert!(write.is_err());
    assert_eq!(destination, [4; 4]);
    assert_eq!(*external.0.lock().expect("external bytes"), [7; 128]);
}

#[tokio::test]
async fn reset_preserves_external_bytes_and_extent() -> Result<(), Box<dyn std::error::Error>> {
    // Given: external bytes attached directly to the device without registration.
    let external = Arc::new(ExternalBytes(Mutex::new([7; 128])));
    let (device, _worker) = AsyncDevice::new(external.clone(), ResourceConfig::default());
    let handle = device.handle();

    // When: the device advances to its next reset epoch.
    handle
        .register_endpoint()
        .write(
            u32::try_from(GlobalRegisters::RESET.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await?;

    // Then: bytes and the backend's zero-length endpoint survive reset.
    let mut bytes = [0; 4];
    handle
        .memory_access()
        .read(MemoryAddress::new(0x8000), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [7; 4]);
    external.validate(MemoryAddress::new(0x8080), MemoryAlignment::ONE, 0)?;
    assert_eq!(
        external.validate(MemoryAddress::new(0x8080), MemoryAlignment::ONE, 1),
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x8080)
        }),
    );
    assert_eq!(*external.0.lock().expect("external bytes"), [7; 128]);
    Ok(())
}

#[test]
fn external_extent_validation_preserves_bytes() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a fixed external extent filled with sentinel bytes.
    let external = ExternalBytes(Mutex::new([7; 128]));
    let address = MemoryAddress::new(0x807f);
    let region = MemoryRegion::try_new(MemoryAddress::new(0x8000), 128, MemoryAlignment::ONE)?;

    // When: validation checks one valid interior range and one crossing range.
    external.validate(MemoryAddress::new(0x8040), MemoryAlignment::ONE, 4)?;
    let crossing = external.validate(address, MemoryAlignment::ONE, 2);

    // Then: it reports the backend's extent without mutating any byte.
    assert_eq!(
        crossing,
        Err(MemoryAccessError::CrossesRegion {
            address,
            length: 2,
            region
        })
    );
    assert_eq!(*external.0.lock().expect("external bytes"), [7; 128]);
    Ok(())
}
