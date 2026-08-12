use std::error::Error;

fn main() {
    let device = pdma_model_device::DmaSpaceError::AddressExhausted;
    let host = pdma_model_host::ModelError::SoftwareCpuInactive;
    let runtime = hardware_interface_runtime::CpuContextError::Inactive;
    println!(
        "device={device:?};host={host:?};runtime={runtime:?};device-source={};host-source={}",
        device.source().is_none(),
        host.source().is_none()
    );
}
