//! Register-only DMA reset without software ring ownership.

use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    CpuContextError, RegisterAccess, SharedMemory, SharedMemoryAccess, SoftwareCpu,
};
use pdma_model_contract::{ChannelBlock, ChannelId, ChannelRegisters};
use pdma_model_dma::{Device, ResetControl};
use std::{
    future::{Future, poll_fn},
    sync::{Arc, Mutex},
    task::Poll,
};

struct Bus(Mutex<Device<SharedMemoryAccess>>);

impl Bus {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self(Mutex::new(Device::new(
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
            .register_offset()
            .ok_or("channel offset overflow")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset();
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
async fn register_only_reset_rejects_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a reset wait suspended on its captured CPU.
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
    // When: a different CPU polls the retained reset.
    let result = other.run_main(reset).await;
    // Then: CPU validation prevents further access and leaves hardware busy.
    assert!(matches!(
        result,
        Err(pdma_model_software::dma::Error::Cpu(
            CpuContextError::Mismatch
        ))
    ));
    assert_eq!(bus.busy(1).await?, 1);
    Ok(())
}

#[tokio::test]
async fn discovery_rejects_a_channel_outside_the_hardware_count()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two advertised hardware channels.
    let bus = Bus::new()?;
    let cpu = SoftwareCpu::new();
    // When: firmware selects the first unavailable channel.
    let result = cpu
        .run_main(pdma_model_software::dma::ChannelControl::discover(
            &bus,
            ChannelId::new(2),
        ))
        .await;
    // Then: discovery reports the advertised limit before reset is written.
    assert!(matches!(
        result,
        Err(pdma_model_software::dma::Error::Channel { available: 2, .. })
    ));
    assert_eq!(bus.busy(0).await?, 0);
    assert_eq!(bus.busy(1).await?, 0);
    Ok(())
}
