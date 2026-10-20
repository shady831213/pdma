mod access;
mod address;
mod backend;
mod bus;
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
pub use bus::{RegisterBus, RegisterEndpointError};
pub use image::{RegisterImage, RegisterImageError};
pub use observer::{
    AccessKind, AccessOrigin, NoopRegisterObserver, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
pub use order::{
    ObservationBackend, ObservationEmissionReceipt, ObservationOrder, ObservationOrderError,
    ObservationReservationReceipt,
};
