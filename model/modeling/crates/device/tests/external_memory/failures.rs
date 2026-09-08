use super::*;
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};
use pdma_model_device::{DeviceCore, PdmaMemoryError, PdmaMemoryMappings};

#[test]
fn constructor_rejects_a_provisioner_bound_to_another_backend() {
    // Given two independent externally owned backends.
    let first = Arc::new(ExternalBytes(Mutex::new([0; 128])));
    let second = Arc::new(ExternalBytes(Mutex::new([0; 128])));
    let core = DeviceCore::new(first, ResourceConfig::default());

    // When runtime construction receives provisioning for the wrong backend.
    let result = AsyncDevice::from_core(core, PdmaMemoryProvisioning::external(second.into()));

    // Then construction rejects the mismatched binding.
    assert!(matches!(result, Err(PdmaMemoryError::InvalidRegion)));
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
async fn reset_preserves_external_mapping_generation_and_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given registered external storage, including a logical empty mapping.
    let external = Arc::new(ExternalBytes(Mutex::new([7; 128])));
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::external(external.into()),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let region = MappedDmaRegion::try_new(BufferId::new(0), DmaAddress::new(0x8000), 4)?;
    let empty = MappedDmaRegion::try_new(BufferId::new(1), DmaAddress::new(0x8040), 0)?;
    handle.register_memory_regions(&[region, empty]).await?;

    // When the device advances to its next reset epoch.
    handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;

    // Then external bytes and logical metadata retain their committed state.
    assert_eq!(handle.memory_generation().await, 1);
    assert_eq!(
        handle.region_for_object(region.object()).await,
        Some(region)
    );
    assert_eq!(handle.read_memory(region.base(), 4).await?, vec![7; 4]);
    assert_eq!(handle.read_memory(empty.base(), 0).await?, Vec::<u8>::new());
    assert_eq!(
        handle.read_memory(empty.base(), 1).await,
        Err(PdmaMemoryError::CrossesRegion)
    );
    Ok(())
}

#[test]
fn invalid_external_registration_keeps_existing_metadata_and_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given existing external bytes and one registered extent.
    let external = Arc::new(ExternalBytes(Mutex::new([7; 128])));
    let mut mappings = PdmaMemoryMappings::new(external.clone());
    let first = MappedDmaRegion::try_new(BufferId::new(0), DmaAddress::new(0x8000), 64)?;
    mappings.register_regions(&[first])?;
    let overlap = MappedDmaRegion::try_new(BufferId::new(1), DmaAddress::new(0x8000), 64)?;

    // When an overlapping second registration is attempted.
    let result = mappings.register_regions(&[overlap]);

    // Then no new identity, generation, or byte mutation commits.
    assert_eq!(result, Err(PdmaMemoryError::OverlappingRegion));
    assert_eq!(mappings.generation(), 1);
    assert_eq!(mappings.mapped_region_count(), 1);
    assert_eq!(*external.0.lock().expect("external bytes"), [7; 128]);
    Ok(())
}

#[tokio::test]
async fn backend_read_fault_publishes_event_without_success_completion()
-> Result<(), Box<dyn std::error::Error>> {
    // Given valid logical mappings whose source bytes are unavailable in the external backend.
    let external = Arc::new(ExternalBytes(Mutex::new([7; 128])));
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::external(external.clone().into()),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let source = MappedDmaRegion::try_new(BufferId::new(0), DmaAddress::new(0x9000), 4)?;
    let destination = MappedDmaRegion::try_new(BufferId::new(1), DmaAddress::new(0x8040), 4)?;
    handle
        .register_memory_regions(&[source, destination])
        .await?;
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await?;
    let worker = tokio::spawn(worker.run());

    // When the device attempts to copy from the unavailable address.
    handle
        .submit_command(
            queue,
            Command::new(
                CommandId::new(0),
                CommandOperation::Copy(CopyRequest::new(source.object(), destination.object(), 4)),
            ),
        )
        .await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_error_after(0).await;

    // Then it publishes the source fault and leaves destination bytes without a success record.
    let event = handle
        .poll_queue_event(queue)
        .await?
        .expect("backend fault event");
    assert_eq!(event.buffer(), source.object());
    assert_eq!(handle.poll_completion(queue).await?, None);
    assert_eq!(*external.0.lock().expect("external bytes"), [7; 128]);
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
