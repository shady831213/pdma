use hardware_interface_runtime::{
    AddressedRegisterAccess, RegisterBusAddress, RegisterBusBase, RegisterBusError,
    RegisterObservation, RegisterObserver,
};
use pdma_model_contract::{abi::mmio::MmioAccessError, semantic::ResourceConfig};
use pdma_model_device::{AsyncDevice, DeviceCore};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::TestResult;

#[derive(Debug, Default)]
struct Observer(Mutex<Vec<RegisterObservation>>);

impl RegisterObserver for Observer {
    fn observe(&self, observation: RegisterObservation) {
        lock(&self.0).push(observation);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn address_adverse_rejects_translation_before_pdma_decode() -> TestResult {
    // Given: one base-aware endpoint and an immutable device/observation snapshot.
    let base = RegisterBusBase::new(0x1_0000_0000);
    let observer = Arc::new(Observer::default());
    let (device, worker) = AsyncDevice::from_core_observed(
        DeviceCore::new(
            Arc::new(hardware_interface_runtime::SharedMemory::new()),
            ResourceConfig::default(),
        ),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle.register_endpoint().at_mmio_base(base);
    let before = handle.snapshot().await;
    let mut response = [0; 9];

    // When: full addresses underflow the base or exceed the u32 offset coordinate space.
    let below = endpoint
        .read_addressed(base, RegisterBusAddress::new(0xffff_fff0), &mut response)
        .await
        .expect_err("below-base address must fail");
    let overflow = endpoint
        .read_addressed(
            base,
            RegisterBusAddress::new(0x2_0000_0000),
            &mut response,
        )
        .await
        .expect_err("unrepresentable offset must fail");
    let mut wrong_width = [0; 4];
    let translated = endpoint
        .read_addressed(
            base,
            RegisterBusAddress::new(base.raw() + 0x010),
            &mut wrong_width,
        )
        .await
        .expect_err("PDMA width must remain exact");

    // Then: runtime translation wins before PDMA decode and valid translation preserves MMIO.
    assert!(matches!(below, RegisterBusError::AddressBelowBase { .. }));
    assert!(matches!(overflow, RegisterBusError::OffsetOutOfRange { delta: 0x1_0000_0000, .. }));
    assert!(matches!(
        &translated,
        RegisterBusError::Endpoint {
            source: pdma_model_device::MmioRegisterError::Access(
                MmioAccessError::WrongWidth {
                    offset: 0x010,
                    expected: 8,
                    actual: 4,
                }
            ),
            ..
        }
    ), "{translated:?}");
    assert_eq!(handle.snapshot().await, before);
    assert!(lock(&observer.0).is_empty());
    assert_eq!(endpoint.mmio_base(), base);
    let mut valid = [0; 4];
    endpoint
        .read_addressed(base, RegisterBusAddress::new(base.raw()), &mut valid)
        .await?;
    assert_eq!(lock(&observer.0).len(), 1);
    println!(
        "address_adverse=true;below_base=typed;offset_overflow=typed;wrong_width=exact;no_forward=true;state_unchanged=true;valid_retry=once"
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
