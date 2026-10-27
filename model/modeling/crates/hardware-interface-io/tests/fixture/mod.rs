use std::convert::Infallible;
use std::future::pending;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use hardware_interface_io::memory::SharedMemory;
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_io::register::observation::{
    AccessOrigin, ObservationGate, RegisterObservation, RegisterObserver,
};

#[derive(Debug)]
pub struct PendingEndpoint {
    pub memory: SharedMemory,
    pub gate: ObservationGate,
    pub committed_writes: AtomicUsize,
}

impl PendingEndpoint {
    pub const fn new(memory: SharedMemory, gate: ObservationGate) -> Self {
        Self {
            memory,
            gate,
            committed_writes: AtomicUsize::new(0),
        }
    }
}

impl RegisterAccess for PendingEndpoint {
    type Error = Infallible;

    async fn read(&self, _offset: u32, _response: &mut [u8]) -> Result<(), Self::Error> {
        pending().await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        pending::<()>().await;
        self.memory
            .write(MemoryAddress::new(0x1000), MemoryAlignment::ONE, request)
            .expect("valid deferred write");
        self.committed_writes.fetch_add(1, Ordering::SeqCst);
        let reservation = self.gate.reserve(1).expect("one deferred sequence");
        let sequence = reservation.start_sequence();
        reservation
            .emit(vec![RegisterObservation::write(
                sequence,
                AccessOrigin::Direct,
                offset,
                request,
            )])
            .expect("deferred observation emits");
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Recorder(pub Mutex<Vec<RegisterObservation>>);

impl RegisterObserver for Recorder {
    fn observe(&self, observation: RegisterObservation) {
        self.0.lock().expect("recorder lock").push(observation);
    }
}

pub fn write(sequence: u64, origin: AccessOrigin, offset: u32) -> RegisterObservation {
    RegisterObservation::write(sequence, origin, offset, &[0xa5])
}

pub(super) fn region(
    base: u64,
    length: u64,
    alignment: u64,
) -> hardware_interface_io::memory::MemoryRegion {
    hardware_interface_io::memory::MemoryRegion::try_new(
        base.into(),
        length,
        MemoryAlignment::try_new(alignment).expect("valid alignment"),
    )
    .expect("valid region")
}
