//! Ordered observation gates and the advanced backend/order implementation protocol.

mod backend;
mod observer;
mod order;

pub use backend::{
    GateFault, LocalObservationGate, LocalObservationReservation, ObservationGate,
    ObservationGateError, ObservationReservation,
};
pub use observer::{
    AccessKind, AccessOrigin, NoopRegisterObserver, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
pub use order::{
    ObservationBackend, ObservationEmissionReceipt, ObservationOrder, ObservationOrderError,
    ObservationReservationReceipt,
};
