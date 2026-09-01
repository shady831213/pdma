use pdma_model_device::AsyncDeviceHandle;
use pdma_model_host::DriverPort;

fn accepts_driver_port<P: DriverPort>() {}

fn main() {
    accepts_driver_port::<AsyncDeviceHandle>();
}
