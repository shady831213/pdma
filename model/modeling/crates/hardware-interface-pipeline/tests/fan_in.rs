//! Readiness-based fan-in contracts.

mod fan_in {
    mod asynchronous;
    mod composition;
    mod first_pending;
    mod lifecycle;
    mod policy;
    mod ready_composition;
    mod selection;
    mod sources;
}
