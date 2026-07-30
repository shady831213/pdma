//! Async PDMA host model.

mod bus;
mod context;
mod driver;
mod irq_delivery;
mod port;

pub use bus::{
    AsyncDeviceBusStage, AsyncPdmaBus, BusDmaStage, BusDmaWrite, BusQueueStatus, BusSubmission,
    QueueControlWrite,
};
pub use context::{CpuLocal, SoftwareCpu};
pub use driver::{AsyncDriver, DriverIrqRecord, ManualDriver};
pub use irq_delivery::{
    HasHandlers, IrqDelivery, IrqDeliveryBootstrap, IrqDeliveryHandle, IrqDeliveryRunner,
    IrqDeliverySummary, IrqHandler, IrqHandlerFuture, IrqHandlerRegistry, NoHandler,
    StatefulIrqHandler, irq_handler, stateful_irq_handler,
};
pub use port::{AsyncDevicePort, BusControlErrorAdapter, BusDevicePort, BusDmaErrorAdapter};
