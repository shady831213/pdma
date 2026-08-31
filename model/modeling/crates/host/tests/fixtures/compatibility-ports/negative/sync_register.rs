use pdma_model_device::compat::SyncDevice;
use pdma_model_host::adapter::abi::PdmaRegisterPort;

fn main() {
    fn reject(device: &SyncDevice) {
        let _ = PdmaRegisterPort::new(device);
    }
    let _ = reject;
}
