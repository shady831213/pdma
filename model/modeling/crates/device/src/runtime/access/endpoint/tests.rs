use super::*;
use crate::AsyncDevice;
use hardware_interface_runtime::{
    GateFault, ObservationGateError, RegisterObservation, RegisterObserver, SharedMemory,
};
use pdma_model_contract::abi::registers::{GlobalRegisters, QueueRegisters, queue_offset};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};

#[tokio::test]
async fn stale_endpoint_write_rejects_before_reservation_or_mutation() {
    // Given: a bound endpoint retained across whole-device reset.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let stale = handle.register_endpoint();
    handle
        .register_endpoint()
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("reset");
    let current = DeviceEpoch::new(1);
    let fresh = stale.at_epoch(current);
    fresh
        .write(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
            &17_u64.to_le_bytes(),
        )
        .await
        .expect("fresh write");
    let sequence = stale.shared.register_gate.next_sequence();

    // When: the stale clone attempts to overwrite a current register.
    let result = stale
        .write(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
            &99_u64.to_le_bytes(),
        )
        .await;

    // Then: rejection consumes neither a register write nor an observer reservation.
    assert_eq!(
        result,
        Err(MmioRegisterError::StaleEpoch {
            expected: current,
            actual: DeviceEpoch::initial()
        })
    );
    assert_eq!(stale.shared.register_gate.next_sequence(), sequence);
    let mut value = [0; 8];
    stale
        .read(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
            &mut value,
        )
        .await
        .expect("non-consuming stale read");
    assert_eq!(u64::from_le_bytes(value), 17);
    stale
        .read(
            u32::try_from(GlobalRegisters::EPOCH.offset()).expect("epoch offset"),
            &mut value,
        )
        .await
        .expect("epoch discovery");
    assert_eq!(u64::from_le_bytes(value), current.raw());
    assert_eq!(fresh.epoch(), current);
    assert_eq!(stale.epoch(), DeviceEpoch::initial());
}

#[derive(Debug)]
struct PanickingObserver;

impl RegisterObserver for PanickingObserver {
    fn observe(&self, _observation: RegisterObservation) {
        panic!("observer emission failure");
    }
}

#[tokio::test]
async fn observer_emission_failure_reports_committed_write_and_publishes_wake() {
    // Given: a real device whose observer fails after a successful register write.
    let (device, _worker) = AsyncDevice::new_observed(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
        Arc::new(PanickingObserver),
    );
    let endpoint = device.handle().register_endpoint();
    let changes = endpoint.shared.backend.subscribe_for_test();

    // When: the first write reaches the observer emission point.
    let result = endpoint
        .write(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
            &17_u64.to_le_bytes(),
        )
        .await;

    // Then: the typed failure identifies committed state and its wake is published.
    let fault = ObservationGateError::Faulted {
        reason: GateFault::ObserverPanicked,
    };
    assert_eq!(result, Err(MmioRegisterError::CommittedObservation(fault)));
    assert!(changes.has_changed().expect("backend is live"));
    let bytes = endpoint
        .shared
        .backend
        .interact(|state| {
            state.core.read_register(
                pdma_model_contract::abi::registers::locate(
                    queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
                )
                .expect("register"),
            )
        })
        .await;
    assert_eq!(bytes, Some(17_u64.to_le_bytes().to_vec()));
}

#[tokio::test]
async fn observer_reservation_failure_rejects_before_write() {
    // Given: an observer gate faulted by a dropped reservation before a write.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let endpoint = device.handle().register_endpoint();
    drop(
        endpoint
            .shared
            .register_gate
            .reserve(1)
            .expect("reservation"),
    );

    // When: a write cannot reserve its observation slot.
    let result = endpoint
        .write(
            queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
            &17_u64.to_le_bytes(),
        )
        .await;

    // Then: the failure is precommit and the register remains at reset.
    assert!(matches!(result, Err(MmioRegisterError::Observation(_))));
    let bytes = endpoint
        .shared
        .backend
        .interact(|state| {
            state.core.read_register(
                pdma_model_contract::abi::registers::locate(
                    queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
                )
                .expect("register"),
            )
        })
        .await;
    assert_eq!(bytes, Some(0_u64.to_le_bytes().to_vec()));
}

#[tokio::test]
async fn constructor_binds_consumed_core_epoch() {
    // Given: a core already reset before being consumed by either runtime constructor.
    let provisioning = PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new()));
    let mut core = DeviceCore::new(provisioning.memory_access(), ResourceConfig::default());
    let epoch = core.reset_runner(DeviceEpoch::initial()).expect("reset");

    // When: the existing core becomes an async runtime.
    let (device, _worker) = AsyncDevice::from_core(core, provisioning).expect("matching backing");
    let endpoint = device.handle().register_endpoint();

    // Then: the endpoint is immediately usable in the consumed core's current epoch.
    assert_eq!(endpoint.epoch(), epoch);
    assert_eq!(
        endpoint
            .write(
                queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base"),
                &17_u64.to_le_bytes()
            )
            .await,
        Ok(())
    );
}
