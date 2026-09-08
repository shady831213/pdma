use crate::DeviceCore;
use crate::queue::exact::PdmaControlError;
use crate::runtime::state::RunnerState;
use pdma_model_contract::abi::mmio::{GlobalCommand, MmioResult};
use pdma_model_contract::semantic::{DeviceEpoch, SemanticError, WholeDeviceReset, ids::BufferId};

#[test]
fn raw_reset_seam_maps_success_and_stale_epoch_exactly() {
    // Given: a runner at its initial epoch.
    let mut state = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );

    // When: the raw reset seam receives the current epoch.
    state.global_registers.reset_expected_epoch = 0.into();
    state.global_registers.command = GlobalCommand::Reset.code().into();
    let success = state.execute_global_command();

    // Then: it advances both identities and returns the new epoch.
    assert!(success);
    assert_eq!(state.global_registers.result.raw(), MmioResult::Ok.code());
    assert_eq!(state.global_registers.response.epoch.raw(), 1);
    assert_eq!(state.epoch(), DeviceEpoch::new(1));

    // When: the same raw epoch is submitted again.
    state.global_registers.command = GlobalCommand::Reset.code().into();
    let stale_outcome = state.execute_global_command();

    // Then: exact stale details are returned before either identity changes.
    assert!(!stale_outcome);
    assert_eq!(
        state.global_registers.result.raw(),
        MmioResult::StaleEpoch.code()
    );
    assert_eq!(state.global_registers.detail0.raw(), 1);
    assert_eq!(state.global_registers.detail1.raw(), 0);
    assert_eq!(state.global_registers.detail2.raw(), 0);
    assert_eq!(state.global_registers.response.epoch.raw(), 1);
    assert_eq!(state.epoch(), DeviceEpoch::new(1));
}

#[test]
fn epoch_overflow_preserves_complete_runner_state() {
    // Given: matching exhausted epochs beside persistent DMA and lifecycle accounting.
    let exhausted = DeviceEpoch::new(u64::MAX);
    let provisioning = crate::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let core = DeviceCore::new(
        provisioning.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let mut state = RunnerState::new(core, provisioning);
    state.core.reset(exhausted);
    let region = state
        .provisioning
        .allocate(&mut state.core.memory, 1)
        .expect("DMA allocation");
    state
        .core
        .memory
        .write_object(region.object(), 0, &[0x6d])
        .expect("DMA write");
    state.steps = 7;
    state.errors = 3;
    state.stalls = 2;
    state.shutdown_requested = true;
    let generation = state.generation;

    // When: reset cannot checked-advance the current epoch.
    let result = state.core.reset_runner(WholeDeviceReset::new(exhausted));

    // Then: the typed overflow leaves identity, DMA, counters, and lifecycle unchanged.
    assert_eq!(
        result,
        Err(PdmaControlError::Semantic(
            SemanticError::DeviceEpochOverflow { current: u64::MAX }
        ))
    );
    assert_eq!(state.core.epoch(), exhausted);
    assert_eq!(
        state.core.memory.read_object(BufferId::new(0), 0, 1),
        Ok(vec![0x6d])
    );
    assert_eq!((state.steps, state.errors, state.stalls), (7, 3, 2));
    assert!(state.shutdown_requested);
    assert_eq!(state.generation, generation);

    // When: the raw reset seam receives the same exhausted expected epoch.
    state.core.global_registers.reset_expected_epoch = u64::MAX.into();
    state.core.global_registers.command = GlobalCommand::Reset.code().into();
    let raw = state.core.execute_global_command();

    // Then: it reports exact overflow details and still commits no mutation.
    assert!(!raw);
    assert_eq!(
        state.core.global_registers.result.raw(),
        MmioResult::DeviceEpochOverflow.code()
    );
    assert_eq!(state.core.global_registers.detail0.raw(), u64::MAX);
    assert_eq!(state.core.global_registers.detail1.raw(), 0);
    assert_eq!(state.core.global_registers.detail2.raw(), 0);
    assert_eq!(state.core.epoch(), exhausted);
    assert_eq!(state.generation, generation);
}
