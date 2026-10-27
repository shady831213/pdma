#![forbid(unsafe_code)]
//! Register transports, schema-backed images and ordered observation policies.

mod access;
mod address;
mod backend;
#[cfg(feature = "schema")]
mod bus;
#[cfg(feature = "schema")]
mod image;
mod observer;
mod order;

pub use access::{AddressedRegisterAccess, RegisterAccess};
pub use address::{
    RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset, RegisterBusWidth,
};
pub use backend::{
    BlockingObservationBackend as ObservationGate,
    BlockingObservationReservation as ObservationReservation,
};
pub use backend::{
    BlockingObservationBackend, BlockingObservationReservation, GateFault, LocalObservationBackend,
    LocalObservationReservation, ObservationGateError,
};
#[cfg(feature = "schema")]
pub use bus::{RegisterBus, RegisterEndpointError};
#[cfg(feature = "schema")]
pub use image::{RegisterImage, RegisterImageError};
pub use observer::{
    AccessKind, AccessOrigin, NoopRegisterObserver, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
pub use order::{
    ObservationBackend, ObservationEmissionReceipt, ObservationOrder, ObservationOrderError,
    ObservationReservationReceipt,
};
