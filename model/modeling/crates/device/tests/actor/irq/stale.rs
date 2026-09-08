use super::Fixture;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    abi::registers::{GlobalRegisters, IrqSourceRegisters, irq_offset},
    semantic::{DeviceEpoch, ResourceConfig},
};
use pdma_model_device::{MmioRegisterError, ModelError};

#[tokio::test(flavor = "current_thread")]
async fn stale_irq_endpoint_and_notification_capability_commit_nothing() {
    // Given: an endpoint whose owning epoch has been reset.
    let (fixture, _worker) = Fixture::new(ResourceConfig::default()).await;
    fixture.enable_irq(0).await;
    let initial = DeviceEpoch::initial();
    fixture
        .endpoint
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset address"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("reset");
    let current = initial.next().expect("epoch");
    let before = fixture.handle.trace_events().await;

    // When: stale writes and notification consumption reach the device boundary.
    for local in [
        IrqSourceRegisters::ROUTE_KIND.offset(),
        IrqSourceRegisters::ENABLED.offset(),
        IrqSourceRegisters::MASKED.offset(),
        IrqSourceRegisters::ACK.offset(),
    ] {
        let result = fixture
            .endpoint
            .write(irq_offset(0, 0, local).expect("IRQ"), &1_u32.to_le_bytes())
            .await;
        assert_eq!(
            result,
            Err(MmioRegisterError::StaleEpoch {
                expected: current,
                actual: initial
            })
        );
    }
    let notification = fixture.handle.take_irq_notification_at_epoch(initial).await;

    // Then: epoch rejection leaves trace and notification state unchanged.
    assert_eq!(
        notification,
        Err(ModelError::StaleDeviceEpoch {
            expected: current,
            actual: initial
        })
    );
    assert_eq!(fixture.handle.trace_events().await, before);
    assert!(!fixture.handle.irq_notification_ready().await);
}
