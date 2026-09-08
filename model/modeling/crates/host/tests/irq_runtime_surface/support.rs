//! State-derived probes behind the Task 23 terminal surface.

pub mod port;
pub mod source;

pub(super) use port::ProbePort;
pub(super) use source::ProbeAccess;

use pdma_model_contract::semantic::IrqNotification;
use pdma_model_host::DriverIrqRecord;
use pdma_model_host::runtime::interrupt::PdmaIrqHandler;
use std::{
    convert::Infallible,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Default)]
pub(super) struct Facts {
    pub turns: usize,
    pub completions: usize,
    pub events: usize,
}

pub(super) struct PersistentHandler {
    facts: Arc<Mutex<Facts>>,
}

impl PersistentHandler {
    pub(super) const fn new(facts: Arc<Mutex<Facts>>) -> Self {
        Self { facts }
    }
}

impl PdmaIrqHandler for PersistentHandler {
    type Error = Infallible;

    async fn handle(
        &mut self,
        _notification: IrqNotification,
        records: &[DriverIrqRecord],
    ) -> Result<(), Self::Error> {
        let mut facts = lock(&self.facts);
        facts.turns = facts.turns.saturating_add(1);
        for record in records {
            match record {
                DriverIrqRecord::Completion(_) => {
                    facts.completions = facts.completions.saturating_add(1);
                }
                DriverIrqRecord::Event(_) => {
                    facts.events = facts.events.saturating_add(1);
                }
            }
        }
        drop(facts);
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct AdverseFacts([bool; 11]);

impl AdverseFacts {
    pub(super) const fn new(values: [bool; 11]) -> Self {
        Self(values)
    }
    pub(super) fn all(&self) -> bool {
        self.0.iter().all(|value| *value)
    }
    pub(super) const fn stale_before_drain(&self) -> bool {
        self.0[0]
    }
    pub(super) const fn unregistered(&self) -> bool {
        self.0[1]
    }
    pub(super) const fn ready_error(&self) -> bool {
        self.0[2]
    }
    pub(super) const fn claim_error(&self) -> bool {
        self.0[3]
    }
    pub(super) const fn shutdown_bias(&self) -> bool {
        self.0[4]
    }
    pub(super) const fn precommit_cancelled(&self) -> bool {
        self.0[5]
    }
    pub(super) const fn accepted_cpu_wait_completed(&self) -> bool {
        self.0[6]
    }
    pub(super) const fn pending_handler_completed(&self) -> bool {
        self.0[7]
    }
    pub(super) const fn drain_error_no_ack(&self) -> bool {
        self.0[8]
    }
    pub(super) const fn handler_error_no_ack(&self) -> bool {
        self.0[9]
    }
    pub(super) const fn ack_error(&self) -> bool {
        self.0[10]
    }
}

pub(super) async fn run_adverse() -> AdverseFacts {
    source::run_adverse().await
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
