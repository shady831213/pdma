#![allow(
    clippy::redundant_pub_crate,
    reason = "included legacy integration modules are crate-private unit-test fixtures"
)]

pub(super) mod pdma_model_host {
    #![allow(
        clippy::redundant_pub_crate,
        unused_imports,
        reason = "retired names are visible only inside the crate-private legacy test harness"
    )]

    pub(super) use super::super::legacy_test_support::*;
    pub(super) use crate::{
        AsyncDriver, DriverIrqRecord, DriverPort, ManualDriver, ModelError, ModelResult,
    };
}

#[path = "../tests/abi_bus.rs"]
mod abi_bus;
#[path = "../tests/bus_device_port.rs"]
mod bus_device_port;
#[path = "../tests/irq_delivery.rs"]
mod irq_delivery;
#[path = "../tests/mmio_bus.rs"]
mod mmio_bus;
#[path = "../tests/mmio_port.rs"]
mod mmio_port;
