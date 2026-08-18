use super::{ConsumerLane, GatedBus};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{BufferId, CommandId, CopyRequest, IrqRoute, IrqVector};
use pdma_model_host::{AsyncDriver, BusDevicePort, DriverPort as AsyncDevicePort, ModelResult};

pub(super) struct PortClones {
    pub(super) main_port: BusDevicePort<GatedBus>,
    pub(super) main_driver: AsyncDriver,
    pub(super) handler_port: BusDevicePort<GatedBus>,
    pub(super) handler_driver: AsyncDriver,
    pub(super) probe_port: BusDevicePort<GatedBus>,
    pub(super) probe_driver: AsyncDriver,
    pub(super) source: BufferId,
    pub(super) destination: BufferId,
}

pub(super) async fn setup_ports(
    bus: &GatedBus,
    cpu: &SoftwareCpu,
    lane: ConsumerLane,
    vector: IrqVector,
) -> ModelResult<PortClones> {
    cpu.run_main(async {
        let mut port = BusDevicePort::new(bus.clone())?;
        let mut driver = AsyncDriver::setup_on(&mut port, 1, 2).await?;
        let source = driver.allocate_buffer_on(&mut port, 4).await?;
        let destination = driver.allocate_buffer_on(&mut port, 4).await?;
        driver
            .write_buffer_on(&mut port, source, &[1, 2, 3, 4])
            .await?;
        let irq = lane.source(&driver);
        port.configure_irq_route(irq, IrqRoute::Message(vector))
            .await?;
        port.enable_irq_source(irq).await?;
        port.unmask_irq_source(irq).await?;
        let len = match lane {
            ConsumerLane::Completion => 4,
            ConsumerLane::Event => 8,
        };
        let command = driver
            .submit_copy_on(&mut port, CopyRequest::new(source, destination, len))
            .await?;
        assert_eq!(command, CommandId::new(0));
        Ok(PortClones {
            main_port: port.clone(),
            main_driver: driver.clone(),
            handler_port: port.clone(),
            handler_driver: driver.clone(),
            probe_port: port,
            probe_driver: driver,
            source,
            destination,
        })
    })
    .await
}
use crate::register::legacy_integration_tests::pdma_model_host;
