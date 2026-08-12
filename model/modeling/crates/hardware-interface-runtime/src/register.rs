mod access;
mod gate;
mod observer;

pub use access::RegisterAccess;
pub use gate::{GateFault, ObservationGate, ObservationGateError, ObservationReservation};
pub use observer::{
    AccessKind, AccessOrigin, NoopRegisterObserver, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
