//! Async PDMA host model.

mod bus;
mod context;
mod driver;
mod error;
mod irq_delivery;
mod port;

pub mod adapters;
pub mod compat;
pub mod core;
pub mod interrupt;
pub mod register;

pub use bus::{
    AsyncDeviceBusStage, AsyncPdmaBus, BusDmaStage, BusDmaWrite, BusQueueStatus, BusSubmission,
    MmioBusControlError, MmioDeviceBus, MmioDeviceTransport, QueueControlWrite,
};
pub use context::{CpuLocal, SoftwareCpu};
pub use driver::{AsyncDriver, DriverIrqRecord, ManualDriver};
pub use error::{ModelError, ModelResult};
pub use irq_delivery::{
    HasHandlers, IrqDelivery, IrqDeliveryBootstrap, IrqDeliveryHandle, IrqDeliveryRunner,
    IrqDeliverySummary, IrqHandler, IrqHandlerFuture, IrqHandlerRegistry, NoHandler,
    StatefulIrqHandler, irq_handler, stateful_irq_handler,
};
pub use port::{
    AsyncDevicePort, BusControlErrorAdapter, BusDevicePort, BusDmaErrorAdapter, DriverPort,
};
