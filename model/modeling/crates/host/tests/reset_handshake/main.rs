//! Reset completion belongs to hardware even when software stops waiting.

pub mod support;

use hw_model_io::register::RegisterAccess;
use hw_model_pipeline::{AsyncInteractionBackend, StepOutcome};
use hw_model_swenv::SoftwareEnv;
use pdma_model_contract::dma::{ChannelId, DmaControl};
use pdma_model_host::DmaError as Error;
use std::{
    future::{Future, poll_fn},
    sync::atomic::Ordering,
    task::Poll,
};
use support::fixture;

#[tokio::test]
async fn reset_stays_pending_after_write_acknowledgement() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: accepted work remains resident while the pipeline is suspended.
    for whole_device in [false, true] {
        let cpu = SoftwareEnv::new();
        let fixture = fixture(&cpu).await?;
        let channel = fixture.channel;
        let reset = async move {
            if whole_device {
                channel.reset_dma().await
            } else {
                channel.reset().await
            }
        };
        tokio::pin!(reset);
        // When: software requests reset and observes its acknowledged write.
        let result = cpu
            .run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
            .await;
        // Then: acknowledgement alone cannot report completed reset.
        assert!(
            result.is_pending(),
            "reset returned before hardware cleanup: {result:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn reset_read_failure_is_returned_to_software() -> Result<(), Box<dyn std::error::Error>> {
    // Given: reset writes succeed but subsequent status reads fail.
    for whole_device in [false, true] {
        let cpu = SoftwareEnv::new();
        let fixture = fixture(&cpu).await?;
        let channel = fixture.channel;
        fixture.fail.store(true, Ordering::SeqCst);
        // When: software waits for reset completion.
        let result = cpu
            .run_main(async move {
                if whole_device {
                    channel.reset_dma().await
                } else {
                    channel.reset().await
                }
            })
            .await;
        // Then: a failed status read is not successful reset completion.
        assert!(matches!(
            result,
            Err(Error::Bus(pdma_model_dma::Error::Unmapped))
        ));
    }
    Ok(())
}

#[tokio::test]
async fn reset_wait_can_migrate_but_still_requires_hardware_completion()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a reset future suspended after its owning CPU requested reset.
    for whole_device in [false, true] {
        let owner = SoftwareEnv::new();
        let other = SoftwareEnv::new();
        let mut fixture = fixture(&owner).await?;
        let channel = fixture.channel;
        let model = fixture.model;
        let reset = async move {
            if whole_device {
                channel.reset_dma().await
            } else {
                channel.reset().await
            }
        };
        tokio::pin!(reset);
        assert!(
            owner
                .run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
                .await
                .is_pending()
        );
        // When: another CPU polls the retained future.
        assert!(
            other
                .run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
                .await
                .is_pending()
        );
        for _ in 0..128 {
            assert!(matches!(
                model
                    .interact(|dma| fixture.driver.step(&mut fixture.hardware, dma))
                    .await?,
                Ok(StepOutcome::InProgress)
            ));
        }
        // Then: switching CPU cannot finish reset early; actual hardware cleanup does.
        other.run_main(reset).await?;
    }
    Ok(())
}

#[tokio::test]
async fn cancelling_reset_wait_preserves_hardware_request() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: reset was accepted and hardware has not run cleanup.
    for whole_device in [false, true] {
        let cpu = SoftwareEnv::new();
        let fixture = fixture(&cpu).await?;
        let channel = fixture.channel;
        let model = fixture.model;
        {
            let reset = async move {
                if whole_device {
                    channel.reset_dma().await
                } else {
                    channel.reset().await
                }
            };
            tokio::pin!(reset);
            assert!(
                cpu.run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
                    .await
                    .is_pending()
            );
            // When: software cancels its pending wait by dropping the future.
        }
        // Then: cancellation does not clear the hardware-owned busy bit.
        let offset = if whole_device {
            (pdma_model_contract::dma::DmaDiscovery::EXTENT
                + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
                + DmaControl::RESET.offset()
        } else {
            ChannelId::new(1)
                .admin_offset(2)
                .ok_or("channel offset overflow")?
        };
        let mut bytes = [0; 4];
        model.read(u32::try_from(offset)?, &mut bytes).await?;
        assert_eq!(u32::from_le_bytes(bytes), 1);
    }
    Ok(())
}

#[tokio::test]
async fn reset_completes_only_after_hardware_clears_its_scope()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: software is waiting while the requested reset scope remains busy.
    for whole_device in [false, true] {
        let cpu = SoftwareEnv::new();
        let mut fixture = fixture(&cpu).await?;
        let channel = fixture.channel;
        let model = fixture.model;
        let reset = async move {
            if whole_device {
                channel.reset_dma().await
            } else {
                channel.reset().await
            }
        };
        tokio::pin!(reset);
        assert!(
            cpu.run_main(poll_fn(|cx| Poll::Ready(reset.as_mut().poll(cx))))
                .await
                .is_pending()
        );
        // When: the retained pipeline advances and cleans up its accepted work.
        for _ in 0..128 {
            assert!(matches!(
                model
                    .interact(|dma| fixture.driver.step(&mut fixture.hardware, dma))
                    .await?,
                Ok(StepOutcome::InProgress)
            ));
        }
        // Then: observing the hardware-cleared bit completes the existing wait.
        cpu.run_main(reset).await?;
    }
    Ok(())
}
