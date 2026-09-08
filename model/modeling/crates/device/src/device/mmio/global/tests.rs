use crate::DeviceCore;
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::mmio::{GlobalCommand, MmioResult};
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::Arc;

#[test]
fn global_command_consumes_owned_registers_and_preserves_response_on_rejection() {
    // Given: the device's global command registers select a valid discovery request.
    let mut core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    core.global_registers.req_abi_version = 1.into();
    core.global_registers.command = GlobalCommand::Discover.code().into();

    // When: the device services its own command register.
    assert!(!core.execute_global_command());

    // Then: discovery succeeds without changing runnable hardware state.
    assert_eq!(core.global_registers.result.raw(), MmioResult::Ok.code());
    assert_eq!(core.global_registers.response.abi_version.raw(), 1);
    let response = core.global_registers.response;

    // Given: an unsupported command follows the successful response.
    core.global_registers.command = u32::MAX.into();
    // When: the device services that register value.
    assert!(!core.execute_global_command());
    // Then: the error retains the last response and reports the actual command.
    assert_eq!(
        core.global_registers.result.raw(),
        MmioResult::UnknownCommand.code()
    );
    assert_eq!(core.global_registers.detail0.raw(), u64::from(u32::MAX));
    assert_eq!(core.global_registers.response, response);
}
