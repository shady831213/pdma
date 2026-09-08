use super::super::support::GatedAccess;
use hardware_interface_runtime::InterruptSource;
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochIrqSource, IrqNotification, WholeDeviceReset,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    DriverPort,
    adapter::abi::{
        AsyncDeviceAccess, PdmaClaimError, PdmaInterruptSource, PdmaIrqAccess, PdmaRegisterPort,
        PdmaRegisterPortError,
    },
};

pub(super) enum Recovery {
    Claim,
    SourceCommand,
    Dispatcher,
    ReadFailure,
    Reset,
    MalformedResult,
    WrongCpu,
    BeforeCommand,
    ObserverFailure,
    SourceCancelled,
    AfterReset,
    SnapshotAfterReset,
    WaitingDispatcher,
    IndependentAdapter,
}

#[expect(clippy::expect_used, reason = "real-device regression assertions")]
impl Recovery {
    pub(super) async fn recover(
        self,
        port: &mut PdmaRegisterPort<GatedAccess>,
        access: &GatedAccess,
        device: &AsyncDevice,
        source: EpochIrqSource,
        offset: u32,
    ) -> Option<IrqNotification> {
        let notification = match self {
            Self::Reset => {
                device
                    .handle()
                    .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
                    .await
                    .expect("reset");
                assert_failed_closed(port, device, 0).await;
                return None;
            }
            Self::MalformedResult => {
                access.corrupt_read(0x224, 0xffff);
                assert_failed_closed(port, device, 0xffff).await;
                access.release();
                return None;
            }
            Self::Dispatcher => dispatch(access.inner()).await,
            Self::IndependentAdapter => {
                let adapter =
                    AsyncDeviceAccess::new(device.handle(), access.irq_coordinator().clone());
                dispatch(adapter).await
            }
            Self::ReadFailure => {
                access.fail_read(offset);
                assert!(
                    Box::pin(port.clone().mask_irq_source(source))
                        .await
                        .is_err()
                );
                access.release();
                claim(port).await
            }
            Self::ObserverFailure => {
                access.fail_observation(offset);
                assert!(matches!(
                    Box::pin(port.try_wait_irq_notification()).await,
                    Err(PdmaRegisterPortError::Claim(PdmaClaimError::Observation(_)))
                ));
                access.release();
                claim(port).await
            }
            Self::AfterReset => {
                use hardware_interface_runtime::RegisterAccess;
                access
                    .inner()
                    .write(0x020, &1_u32.to_le_bytes())
                    .await
                    .expect("discover");
                let mut global_epoch = [0; 8];
                access
                    .inner()
                    .read(0x050, &mut global_epoch)
                    .await
                    .expect("latched epoch");
                assert_eq!(u64::from_le_bytes(global_epoch), 0);
                let notification = claim(port).await;
                assert_eq!(notification.epoch(), DeviceEpoch::new(1));
                notification
            }
            Self::SnapshotAfterReset => {
                assert!(
                    Box::pin(port.try_irq_snapshot())
                        .await
                        .expect("empty snapshot")
                        .is_none()
                );
                let notification = claim(port).await;
                assert_eq!(notification.epoch(), DeviceEpoch::new(1));
                notification
            }
            Self::SourceCommand | Self::SourceCancelled => {
                port.clone()
                    .mask_irq_source(source)
                    .await
                    .expect("recover before mask");
                claim(port).await
            }
            Self::Claim | Self::WrongCpu | Self::BeforeCommand | Self::WaitingDispatcher => {
                claim(port).await
            }
        };
        Some(notification)
    }
}

#[expect(clippy::expect_used, reason = "real-device regression assertions")]
async fn claim(port: &mut PdmaRegisterPort<GatedAccess>) -> IrqNotification {
    Box::pin(port.try_wait_irq_notification())
        .await
        .expect("recover")
        .expect("message")
}

#[expect(clippy::expect_used, reason = "real-device regression assertions")]
async fn dispatch(access: AsyncDeviceAccess) -> IrqNotification {
    let mut source = PdmaInterruptSource::new(access);
    let ready = source.wait_ready().await.expect("retained ready");
    Box::pin(source.claim(ready))
        .await
        .expect("claim")
        .expect("message")
        .notification()
}

#[expect(clippy::expect_used, reason = "real-device regression assertions")]
async fn assert_failed_closed(
    port: &mut PdmaRegisterPort<GatedAccess>,
    device: &AsyncDevice,
    code: u64,
) {
    assert!(matches!(Box::pin(port.try_wait_irq_notification()).await,
        Err(PdmaRegisterPortError::Claim(PdmaClaimError::CommandResult { code: actual })) if actual == code));
    device.request_shutdown().await;
    assert!(
        Box::pin(port.try_wait_irq_notification())
            .await
            .expect("closed")
            .is_none()
    );
}
