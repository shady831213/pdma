//! Reset completion belongs to hardware even when software stops waiting.

use hardware_interface_pipeline::{AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{
    CpuContextError, RegisterAccess, SharedMemory, SharedMemoryAccess, SoftwareCpu,
};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, DeviceControl, DeviceRegisters,
};
use pdma_model_dma::{Device, ResetControl};
use pdma_model_host::{Channel, ChannelConfig, Error, RingConfig, Rings, runtime::Model};
use std::{
    future::{Future, poll_fn},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
};

struct ReadFailureBus {
    model: Model<SharedMemoryAccess>,
    fail: Arc<AtomicBool>,
}

impl RegisterAccess for ReadFailureBus {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(pdma_model_dma::Error::Unmapped);
        }
        self.model.read(offset, response).await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.model.write(offset, request).await
    }
}

async fn fixture(
    cpu: &SoftwareCpu,
) -> Result<
    (
        Channel<ReadFailureBus, SharedMemoryAccess>,
        Model<SharedMemoryAccess>,
        Arc<AtomicBool>,
    ),
    Box<dyn std::error::Error>,
> {
    let memory = SharedMemoryAccess::from(Arc::new(SharedMemory::new()));
    let model = Model::new(Device::new(memory.clone(), 2)?);
    let fail = Arc::new(AtomicBool::new(false));
    let channel = cpu
        .run_main(Channel::setup(
            ReadFailureBus {
                model: model.clone(),
                fail: Arc::clone(&fail),
            },
            memory,
            ChannelConfig {
                id: ChannelId::new(1),
                rings: Rings {
                    command: RingConfig::new(0x1000, 1)?,
                    response: RingConfig::new(0x2000, 1)?,
                },
            },
        ))
        .await?;
    Ok((channel, model, fail))
}

#[tokio::test]
async fn reset_stays_pending_after_write_acknowledgement() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: no hardware operation is running to finish either reset scope.
    for whole_device in [false, true] {
        let cpu = SoftwareCpu::new();
        let (channel, _model, _fail) = fixture(&cpu).await?;
        let reset = async move {
            if whole_device {
                channel.reset_device().await
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
        let cpu = SoftwareCpu::new();
        let (channel, _model, fail) = fixture(&cpu).await?;
        fail.store(true, Ordering::SeqCst);
        // When: software waits for reset completion.
        let result = cpu
            .run_main(async move {
                if whole_device {
                    channel.reset_device().await
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
async fn reset_wait_rejects_repoll_on_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a reset future suspended after its owning CPU requested reset.
    for whole_device in [false, true] {
        let owner = SoftwareCpu::new();
        let other = SoftwareCpu::new();
        let (channel, _model, _fail) = fixture(&owner).await?;
        let reset = async move {
            if whole_device {
                channel.reset_device().await
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
        let result = other.run_main(reset).await;
        // Then: the next register read enforces the original CPU ownership.
        assert!(matches!(result, Err(Error::Cpu(CpuContextError::Mismatch))));
    }
    Ok(())
}

#[tokio::test]
async fn cancelling_reset_wait_preserves_hardware_request() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: reset was accepted and hardware has not run cleanup.
    for whole_device in [false, true] {
        let cpu = SoftwareCpu::new();
        let (channel, model, _fail) = fixture(&cpu).await?;
        {
            let reset = async move {
                if whole_device {
                    channel.reset_device().await
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
            DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset()
        } else {
            ChannelId::new(1)
                .register_offset()
                .ok_or("channel offset overflow")?
                + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RESET.offset()
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
        let cpu = SoftwareCpu::new();
        let (channel, model, _fail) = fixture(&cpu).await?;
        let reset = async move {
            if whole_device {
                channel.reset_device().await
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
        // When: the independent hardware driver executes idle reset cleanup.
        let outcome = model
            .interact(|device| {
                assert!(matches!(
                    Operation::new(()).step(&mut ResetControl, device),
                    StepOutcome::Progressed
                ));
                StepOutcome::<(), std::convert::Infallible, ()>::Complete(Ok(()))
            })
            .await?;
        assert!(matches!(outcome, StepOutcome::Complete(Ok(()))));
        // Then: observing the hardware-cleared bit completes the existing wait.
        cpu.run_main(reset).await?;
    }
    Ok(())
}
