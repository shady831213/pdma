pub const SYMBOLS: &[(&str, &str)] = &[
    (
        "AsyncDevicePort",
        "pdma_model_host::DriverPort or explicit compat adapter",
    ),
    (
        "AsyncPdmaBus",
        "pdma_model_host::register::PdmaRegisterPort",
    ),
    (
        "BusDevicePort",
        "pdma_model_host::register::PdmaRegisterPort",
    ),
    (
        "MmioDeviceBus",
        "pdma_model_host::register::PdmaRegisterPort",
    ),
    (
        "MmioDeviceTransport",
        "pdma_model_host::adapters::AsyncDeviceAccess",
    ),
    ("BusQueueStatus", "canonical register procedure result"),
    (
        "AsyncDeviceBusStage",
        "pdma_model_host::register::PdmaPreparedProposal",
    ),
    (
        "BusSubmission",
        "pdma_model_host::register::PdmaBatchAccess",
    ),
    ("QueueControlWrite", "canonical register procedure"),
    (
        "BusDmaStage",
        "pdma_model_host::register::PdmaPreparedProposal",
    ),
    ("BusDmaWrite", "hardware_interface_runtime::MemoryAccess"),
    (
        "MmioBusControlError",
        "pdma_model_host::register::RegisterPortError",
    ),
    ("BusControlErrorAdapter", "layer-owned host adapter error"),
    ("BusDmaErrorAdapter", "layer-owned host adapter error"),
    ("CpuLocal", "hardware_interface_runtime::CpuLocal"),
    ("SoftwareCpu", "hardware_interface_runtime::SoftwareCpu"),
    ("HasHandlers", "hardware_interface_runtime::HandlerRegistry"),
    ("IrqDelivery", "pdma_model_host::interrupt::PdmaIrqDelivery"),
    (
        "IrqDeliveryBootstrap",
        "hardware_interface_runtime::HandlerRegistry",
    ),
    (
        "IrqDeliveryHandle",
        "hardware_interface_runtime::InterruptShutdownHandle",
    ),
    (
        "IrqDeliveryRunner",
        "hardware_interface_runtime::InterruptDispatcherTask",
    ),
    ("IrqDeliverySummary", "runtime dispatcher join result"),
    ("IrqHandler", "hardware_interface_runtime::InterruptHandler"),
    (
        "IrqHandlerFuture",
        "hardware_interface_runtime::InterruptHandler future",
    ),
    (
        "IrqHandlerRegistry",
        "hardware_interface_runtime::HandlerRegistry",
    ),
    ("NoHandler", "hardware_interface_runtime::HandlerRegistry"),
    (
        "StatefulIrqHandler",
        "hardware_interface_runtime::StatefulInterruptHandler",
    ),
    (
        "irq_handler",
        "hardware_interface_runtime::interrupt_handler",
    ),
    (
        "stateful_irq_handler",
        "hardware_interface_runtime::stateful_interrupt_handler",
    ),
    ("SyncDevice", "pdma_model_device::compat::SyncDevice"),
];
