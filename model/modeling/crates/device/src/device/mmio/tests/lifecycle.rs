use std::sync::Arc;

use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::mmio::{MmioAccess, PdmaMmioMap};
use pdma_model_contract::semantic::ResourceConfig;

use crate::{AsyncDevice, DeviceCore, PdmaMemoryProvisioning};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test(flavor = "current_thread")]
async fn spawned_actor_handles_share_register_state_across_reset() -> TestResult {
    // Given: one core transfers its register owners into a spawned actor.
    let provisioning = PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new()));
    let core = DeviceCore::new(provisioning.memory_access(), ResourceConfig::default());
    let (device, worker) = AsyncDevice::from_core(core, provisioning)?;
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let observer = handle.clone();
    // When: every readable register is accessed and request bytes are changed.
    for register in PdmaMmioMap::resolve()?.registers() {
        let definition = register.definition();
        if definition.access() != MmioAccess::WriteOnly {
            let bytes = handle
                .read_mmio(
                    u32::try_from(register.offset())?,
                    definition.width() as u128,
                )
                .await?;
            assert_eq!(
                bytes,
                definition
                    .reset_value()
                    .to_le_bytes()
                    .get(..definition.width())
                    .ok_or("width")?
            );
        }
    }
    handle.write_mmio(0x010, &17_u64.to_le_bytes()).await?;
    assert_eq!(observer.read_mmio(0x010, 8).await?, 17_u64.to_le_bytes());
    // Then: reset clears the same requests and advances the epoch seen by both handles.
    handle.write_mmio(0x020, &4_u32.to_le_bytes()).await?;
    assert_eq!(handle.read_mmio(0x010, 8).await?, 0_u64.to_le_bytes());
    assert_eq!(handle.read_mmio(0x050, 8).await?, 1_u64.to_le_bytes());
    assert_eq!(observer.read_mmio(0x010, 8).await?, 0_u64.to_le_bytes());
    assert_eq!(observer.read_mmio(0x050, 8).await?, 1_u64.to_le_bytes());
    device.request_shutdown().await;
    worker.await?;
    drop(handle);
    drop(device);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn global_command_response_matches_the_owner_typed_values() -> TestResult {
    // Given: a real device endpoint.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    // When: READ_LIMITS replaces the response through the raw command endpoint.
    handle.write_mmio(0x020, &2_u32.to_le_bytes()).await?;
    let typed = handle
        .shared
        .backend
        .interact(|state| {
            let response = &state.core.global_registers.response;
            [
                u64::from(response.abi_version.raw()),
                u64::from(response.run_state.raw()),
                response.features.raw(),
                response.epoch.raw(),
                u64::from(response.max_ring_depth.raw()),
                u64::from(response.max_indirect_destinations.raw()),
                u64::from(response.max_transfer_bytes.raw()),
                u64::from(response.max_strided_rows.raw()),
                u64::from(response.max_stride.raw()),
            ]
        })
        .await;
    // Then: every raw response leaf is the corresponding typed leaf's bytes.
    for ((offset, width), value) in [
        (0x040, 4),
        (0x044, 4),
        (0x048, 8),
        (0x050, 8),
        (0x058, 4),
        (0x05c, 4),
        (0x060, 4),
        (0x064, 4),
        (0x068, 4),
    ]
    .into_iter()
    .zip(typed)
    {
        assert_eq!(
            handle.read_mmio(offset, width).await?,
            value
                .to_le_bytes()
                .get(..usize::try_from(width)?)
                .ok_or("width")?
        );
    }
    Ok(())
}
