use std::sync::Arc;

use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::mmio::{MmioAccess, PdmaMmioMap};
use pdma_model_contract::semantic::ResourceConfig;

use super::allocation_observation;
use crate::{AsyncDevice, DeviceCore};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test(flavor = "current_thread")]
async fn spawned_actor_keeps_one_image_until_reset_and_drop() -> TestResult {
    // Given: one core transfers its register image into a spawned actor.
    let before = allocation_observation();
    let core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    assert_eq!(allocation_observation(), (before.0 + 1, before.1 + 1));
    let (device, worker) = AsyncDevice::from_core(core);
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
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
    // Then: reads and writes reuse that image; reset replaces it with one live image.
    assert_eq!(allocation_observation(), (before.0 + 1, before.1 + 1));
    handle.write_mmio(0x020, &4_u32.to_le_bytes()).await?;
    assert_eq!(allocation_observation(), (before.0 + 2, before.1 + 1));
    assert_eq!(handle.read_mmio(0x010, 8).await?, 0_u64.to_le_bytes());
    assert_eq!(handle.read_mmio(0x050, 8).await?, 1_u64.to_le_bytes());
    device.request_shutdown().await;
    worker.await?;
    drop(handle);
    drop(device);
    assert_eq!(allocation_observation(), (before.0 + 2, before.1));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn global_command_response_matches_the_live_typed_image() -> TestResult {
    // Given: a real device endpoint.
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let handle = device.handle();
    // When: READ_LIMITS replaces the response through the raw command endpoint.
    handle.write_mmio(0x020, &2_u32.to_le_bytes()).await?;
    let typed = handle
        .shared
        .backend
        .interact(|state| {
            state.core.registers.view(
                |root| -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
                    let global = root.global()?;
                    Ok([
                        global.rsp_abi_version()?.value()?,
                        global.rsp_run_state()?.value()?,
                        global.rsp_features()?.value()?,
                        global.rsp_epoch()?.value()?,
                        global.max_ring_depth()?.value()?,
                        global.max_indirect_destinations()?.value()?,
                        global.max_transfer_bytes()?.value()?,
                        global.max_strided_rows()?.value()?,
                        global.max_stride()?.value()?,
                    ])
                },
            )
        })
        .await?;
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
