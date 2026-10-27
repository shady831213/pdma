//! Register-only DMA reset without software ring ownership.

pub mod support;

use hardware_interface_register::RegisterAccess;
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::dma::ChannelId;
use std::{
    future::{Future, poll_fn},
    task::Poll,
};

use support::{Bus, Relocated};

#[tokio::test]
async fn ring_context_setter_selects_the_requested_channel()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two reset channels with independent ring selectors.
    let bus = Bus::new()?;
    let control =
        pdma_model_software::dma::ChannelControl::discover(&bus, ChannelId::new(1)).await?;
    // When: software selects a context before enabling its DMA ring.
    control
        .set_ring_context(&bus, pdma_model_contract::dma::ContextId::new(37))
        .await?;
    // Then: the selected channel uses that selector and its peer retains zero.
    for (index, expected) in [(0, 0), (1, 37)] {
        let offset = ChannelId::new(index).register_offset(2).ok_or("channel")?
            + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
            + pdma_model_contract::dma::ChannelRegisters::CONFIG.offset()
            + pdma_model_contract::dma::ChannelConfiguration::RING_CONTEXT.offset();
        let mut bytes = [0; 4];
        bus.read(u32::try_from(offset)?, &mut bytes).await?;
        assert_eq!(u32::from_le_bytes(bytes), expected);
    }
    Ok(())
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
            let peer = pdma_model_software::dma::ChannelControl::discover_at(
                &bus,
                ChannelId::new(0),
                0x4000,
            )
            .await?;
            peer.set_ring_context(&bus, pdma_model_contract::dma::ContextId::new(37))
                .await?;
            control
                .set_ring_context(&bus, pdma_model_contract::dma::ContextId::new(19))
                .await?;
            // When: software controls the selected data, administration and global apertures.
            control.set_irq_enabled(&bus, true).await?;
            control.set_dma_irq_enabled(&bus, true).await?;
            control.request_reset(&bus).await?;
            bus.0.0.lock().map_err(|_| "poisoned DMA")?.signal_fatal();
            // Then: the cached addresses select the intended scope and preserve its peer.
            assert!(!control.reset_in_progress(&bus).await?);
            assert_eq!(bus.0.busy(0).await?, 0);
            assert_eq!(bus.0.busy(1).await?, 0);
            assert_eq!(bus.0.ring_context(0).await?, 37);
            assert_eq!(bus.0.ring_context(1).await?, 0);
            assert_eq!(
                control.dma_fatal(&bus).await?,
                pdma_model_contract::dma::DmaFatalCode::Internal
            );
            assert!(bus.0.0.lock().map_err(|_| "poisoned DMA")?.take_interrupt());
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn reset_status_query_does_not_wait_or_reset_peer() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: channel one has requested reset on a real two-channel DMA core.
    let (bus, mut hardware, mut driver) = Bus::resident()?;
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
            bus.advance(&mut hardware, &mut driver)?;
            assert!(!control.reset_in_progress(&bus).await?);
            assert_eq!(bus.ring_context(0).await?, 37);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn register_only_reset_waits_for_hardware_and_preserves_other_channel()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: firmware has a register bus without allocating either DMA ring.
    let (bus, mut hardware, mut driver) = Bus::resident()?;
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
    bus.advance(&mut hardware, &mut driver)?;
    cpu.run_main(reset).await?;
    assert_eq!(bus.busy(1).await?, 0);
    assert_eq!(bus.ring_context(0).await?, 37);
    Ok(())
}

#[tokio::test]
async fn cancelling_register_only_reset_does_not_fake_drain()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: firmware has no ring owner to consume during reset.
    let (bus, mut hardware, mut driver) = Bus::resident()?;
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
    bus.advance(&mut hardware, &mut driver)?;
    cpu.run_main(control.wait_reset(&bus)).await?;
    assert_eq!(bus.ring_context(0).await?, 37);
    Ok(())
}

#[tokio::test]
async fn register_only_reset_completes_on_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a reset wait suspended on the first CPU.
    let (bus, mut hardware, mut driver) = Bus::resident()?;
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
    bus.advance(&mut hardware, &mut driver)?;
    other.run_main(reset).await?;
    // Then: reset completion is visible outside a CPU and the peer stayed idle.
    assert!(!control.reset_in_progress(&bus).await?);
    assert_eq!(bus.busy(0).await?, 0);
    assert_eq!(bus.ring_context(0).await?, 37);
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
