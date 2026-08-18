use hardware_interface_runtime::{CpuLocal, MemoryAccess, SoftwareCpu};
use pdma_model_contract::{abi::DmaAddress, bridge::PdmaMemoryRegion, semantic::BufferId};
use pdma_model_host::{
    DriverPort,
    adapters::AsyncDeviceAccess,
    compat::{AsyncDevicePort, SemanticDevicePort},
    interrupt::PdmaIrqDelivery,
    register::PdmaRegisterPort,
};

fn traits<T: DriverPort, M: MemoryAccess>() {
    let _ = std::marker::PhantomData::<(T, M)>;
}

fn main() {
    let _ = std::mem::size_of::<Option<AsyncDeviceAccess>>();
    let _ = std::mem::size_of::<Option<PdmaRegisterPort<AsyncDeviceAccess>>>();
    let _ = std::mem::size_of::<Option<AsyncDevicePort>>();
    let _ = std::mem::size_of::<Option<SemanticDevicePort<'static>>>();
    let _ = std::mem::size_of::<PdmaIrqDelivery>();
    let _ = std::mem::size_of::<Option<CpuLocal<u8>>>();
    let _ = SoftwareCpu::new;
    let _ = DmaAddress::new(0);
    let _ = BufferId::new(0);
    let _ = std::mem::size_of::<PdmaMemoryRegion>();
}
