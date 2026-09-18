//! Register-only DMA reset without software ring ownership.

use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{RegisterAccess, SharedMemory, SharedMemoryAccess, SoftwareCpu};
use pdma_model_contract::ChannelId;
use pdma_model_dma::{Dma, ResetControl};
use std::{
    future::{Future, poll_fn},
    sync::{Arc, Mutex},
    task::Poll,
};

struct Bus(Mutex<Dma<SharedMemoryAccess>>);

struct Relocated(Bus);

impl RegisterAccess for Relocated {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0
            .read(
                offset.checked_sub(0x4000).ok_or(Self::Error::Unmapped)?,
                bytes,
            )
            .await
    }

    async fn write(&self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0
            .write(
                offset.checked_sub(0x4000).ok_or(Self::Error::Unmapped)?,
                bytes,
            )
            .await
    }
}

#[tokio::test]
async fn relocated_discovery_routes_data_admin_and_global_controls()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a two-channel DMA aperture at a nonzero bus address.
    let bus = Relocated(Bus::new()?);
    SoftwareCpu::new()
        .run_main(async {
            let control = pdma_model_software::dma::ChannelControl::discover_at(
                &bus,
                ChannelId::new(1),
                0x4000,
            )
            .await?;
            // When: software controls the selected data, administration and global apertures.
            control.set_irq_enabled(&bus, true).await?;
            control.set_dma_irq_enabled(&bus, true).await?;
            control.request_reset(&bus).await?;
            bus.0.0.lock().map_err(|_| "poisoned DMA")?.signal_fatal();
            // Then: the cached addresses select the intended scope and preserve its peer.
            assert!(control.reset_in_progress(&bus).await?);
            assert_eq!(bus.0.busy(0).await?, 0);
            assert_eq!(bus.0.busy(1).await?, 1);
            assert_eq!(
                control.dma_fault(&bus).await?,
                pdma_model_contract::DmaFaultCode::Internal
            );
            assert!(bus.0.0.lock().map_err(|_| "poisoned DMA")?.take_interrupt());
            bus.0.finish_reset()?;
            assert!(!control.reset_in_progress(&bus).await?);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn reset_status_query_does_not_wait_or_reset_peer() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: channel one has requested reset on a real two-channel DMA core.
    let bus = Bus::new()?;
    SoftwareCpu::new()
        .run_main(async {
            let control =
                pdma_model_software::dma::ChannelControl::discover(&bus, ChannelId::new(1)).await?;
            control.request_reset(&bus).await?;
            // When: firmware samples progress without waiting for cleanup.
            assert!(control.reset_in_progress(&bus).await?);
            // Then: the query leaves the request and peer intact until hardware finishes.
            assert_eq!(bus.busy(0).await?, 0);
            assert_eq!(bus.busy(1).await?, 1);
            bus.finish_reset()?;
            assert!(!control.reset_in_progress(&bus).await?);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

impl Bus {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self(Mutex::new(Dma::new(
            SharedMemoryAccess::from(Arc::new(SharedMemory::new())),
            2,
        )?)))
    }

    fn finish_reset(&self) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            Operation::new(()).step(
                &mut ResetControl,
                &mut *self.0.lock().map_err(|_| "poisoned device lock")?
            ),
            StepOutcome::Progressed
        ));
        Ok(())
    }

    async fn busy(&self, channel: usize) -> Result<u32, Box<dyn std::error::Error>> {
        let offset = ChannelId::new(channel)
            .admin_offset(2)
            .ok_or("channel offset overflow")?;
        let mut bytes = [0; 4];
        self.read(u32::try_from(offset)?, &mut bytes).await?;
        Ok(u32::from_le_bytes(bytes))
    }
}

impl RegisterAccess for Bus {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.lock().map_err(|_| Self::Error::Unmapped)?.read(
            usize::try_from(offset).map_err(|_| Self::Error::Unmapped)?,
            bytes,
        )
    }

    async fn write(&self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.lock().map_err(|_| Self::Error::Unmapped)?.write(
            usize::try_from(offset).map_err(|_| Self::Error::Unmapped)?,
            bytes,
        )
    }
}

#[tokio::test]
async fn register_only_reset_waits_for_hardware_and_preserves_other_channel()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: firmware has a register bus without allocating either DMA ring.
    let bus = Bus::new()?;
    let cpu = SoftwareCpu::new();
    let control = cpu
        .run_main(pdma_model_software::dma::ChannelControl::discover(
            &bus,
            ChannelId::new(1),
        ))
        .await?;
    let reset = control.reset(&bus);
    tokio::pin!(reset);
    // When: firmware requests reset before hardware performs cleanup.
    assert!(
        cpu.run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
            .await
            .is_pending()
    );
    // Then: only the selected channel is busy, and hardware completion releases the wait.
    assert_eq!(bus.busy(1).await?, 1);
    assert_eq!(bus.busy(0).await?, 0);
    bus.finish_reset()?;
    cpu.run_main(reset).await?;
    assert_eq!(bus.busy(1).await?, 0);
    Ok(())
}

#[tokio::test]
async fn cancelling_register_only_reset_does_not_fake_drain()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: firmware has no ring owner to consume during reset.
    let bus = Bus::new()?;
    let cpu = SoftwareCpu::new();
    let control = cpu
        .run_main(pdma_model_software::dma::ChannelControl::discover(
            &bus,
            ChannelId::new(1),
        ))
        .await?;
    // When: firmware drops the pending reset wait.
    {
        let reset = control.reset(&bus);
        tokio::pin!(reset);
        assert!(
            cpu.run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
                .await
                .is_pending()
        );
    }
    // Then: hardware retains the request until its own cleanup runs.
    assert_eq!(bus.busy(1).await?, 1);
    bus.finish_reset()?;
    cpu.run_main(control.wait_reset(&bus)).await?;
    Ok(())
}

#[tokio::test]
async fn register_only_reset_completes_on_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a reset wait suspended on the first CPU.
    let bus = Bus::new()?;
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let control = owner
        .run_main(pdma_model_software::dma::ChannelControl::discover(
            &bus,
            ChannelId::new(1),
        ))
        .await?;
    let reset = control.reset(&bus);
    tokio::pin!(reset);
    assert!(
        owner
            .run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
            .await
            .is_pending()
    );
    // When: hardware completes and a different CPU polls the retained reset.
    bus.finish_reset()?;
    other.run_main(reset).await?;
    // Then: reset completion is visible outside a CPU and the peer stayed idle.
    assert!(!control.reset_in_progress(&bus).await?);
    assert_eq!(bus.busy(0).await?, 0);
    Ok(())
}

#[tokio::test]
async fn discovery_rejects_a_channel_outside_the_hardware_count()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two advertised hardware channels.
    let bus = Bus::new()?;
    // When: firmware discovers an available channel without a modeled CPU.
    let control =
        pdma_model_software::dma::ChannelControl::discover(&bus, ChannelId::new(1)).await?;
    assert!(!control.reset_in_progress(&bus).await?);
    let result = pdma_model_software::dma::ChannelControl::discover(&bus, ChannelId::new(2)).await;
    // Then: discovery reports the advertised limit before reset is written.
    assert!(matches!(
        result,
        Err(pdma_model_software::dma::Error::Channel { available: 2, .. })
    ));
    assert_eq!(bus.busy(0).await?, 0);
    assert_eq!(bus.busy(1).await?, 0);
    Ok(())
}
