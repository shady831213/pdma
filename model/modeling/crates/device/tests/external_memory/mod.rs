use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::{
    BufferId, Command, CommandId, CommandOperation, QueueConfig, ResourceConfig, SoftwareOwnerId,
    desc::CopyRequest, queue::EventRouting,
};
use pdma_model_device::{AsyncDevice, MappedDmaRegion, PdmaMemoryProvisioning};
use std::sync::{Arc, Mutex};

mod exact_abi;
mod failures;

struct ExternalBytes<const N: usize = 128>(Mutex<[u8; N]>);

impl<const N: usize> MemoryAccess for ExternalBytes<N> {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        if address.raw() % alignment.raw() != 0 {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        let offset = usize::try_from(
            address
                .raw()
                .checked_sub(0x8000)
                .ok_or(MemoryAccessError::Unmapped { address })?,
        )
        .map_err(|_| MemoryAccessError::Unmapped { address })?;
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source = bytes
            .get(offset..offset.saturating_add(destination.len()))
            .ok_or(MemoryAccessError::Unmapped { address })?;
        destination.copy_from_slice(source);
        drop(bytes);
        Ok(())
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        if address.raw() % alignment.raw() != 0 {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        let offset = usize::try_from(
            address
                .raw()
                .checked_sub(0x8000)
                .ok_or(MemoryAccessError::Unmapped { address })?,
        )
        .map_err(|_| MemoryAccessError::Unmapped { address })?;
        let mut bytes = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let destination = bytes
            .get_mut(offset..offset.saturating_add(source.len()))
            .ok_or(MemoryAccessError::Unmapped { address })?;
        destination.copy_from_slice(source);
        drop(bytes);
        Ok(())
    }
}

#[tokio::test]
async fn device_copies_externally_provisioned_bytes_on_injected_backend()
-> Result<(), Box<dyn std::error::Error>> {
    // Given existing external storage with no allocation or installation capability.
    let external = Arc::new(ExternalBytes(Mutex::new([0; 128])));
    external.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &[9, 8, 7, 6],
    )?;
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::external(external.clone().into()),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let source = MappedDmaRegion::try_new(BufferId::new(0), DmaAddress::new(0x8000), 4)?;
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

    // When the actual worker executes a semantic copy using those registered addresses.
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
    device.wait_for_progress_after(0).await;

    // Then the external owner and host access capability observe the device's bytes.
    let mut bytes = [0; 4];
    handle
        .memory_access()
        .read(MemoryAddress::new(0x8040), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [9, 8, 7, 6]);
    assert_eq!(
        external.0.lock().expect("external bytes").get(64..68),
        Some([9, 8, 7, 6].as_slice())
    );
    assert!(handle.poll_completion(queue).await?.is_some());
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
