#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

use hardware_interface_io::register::RegisterAccess;
use hardware_interface_io::register::observation::{
    AccessKind, AccessOrigin, ObservationGate, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::{Future, pending};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Debug)]
struct EndpointCause;

impl Display for EndpointCause {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("literal endpoint cause")
    }
}

impl Error for EndpointCause {}

#[derive(Debug)]
struct EndpointError {
    cause: EndpointCause,
}

impl Display for EndpointError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("register endpoint rejected access")
    }
}

impl Error for EndpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

#[derive(Debug, Default)]
struct RecordingObserver {
    observations: Mutex<Vec<RegisterObservation>>,
}

impl RegisterObserver for RecordingObserver {
    fn observe(&self, observation: RegisterObservation) {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointMode {
    Ready,
    Error,
    Pending,
}

#[derive(Debug)]
struct ToyEndpoint {
    mode: EndpointMode,
    gate: ObservationGate,
    writes: Mutex<Vec<(u64, Box<[u8]>)>>,
}

impl ToyEndpoint {
    fn new(mode: EndpointMode, observer: SharedRegisterObserver) -> Self {
        Self {
            mode,
            gate: ObservationGate::new(observer),
            writes: Mutex::new(Vec::new()),
        }
    }

    fn committed_writes(&self) -> usize {
        self.writes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

impl RegisterAccess for ToyEndpoint {
    type Error = EndpointError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        if response.len() != 4 || self.mode == EndpointMode::Error {
            return Err(EndpointError {
                cause: EndpointCause,
            });
        }
        response.copy_from_slice(&[0x78, 0x56, 0x34, 0x12]);
        let reservation = self.gate.reserve(1).expect("one sequence");
        let observation = RegisterObservation::read(
            reservation.start_sequence(),
            AccessOrigin::Direct,
            offset,
            response,
        );
        reservation.emit(vec![observation]).expect("emit");
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        if request.len() != 4 {
            return Err(EndpointError {
                cause: EndpointCause,
            });
        }
        match self.mode {
            EndpointMode::Ready => {}
            EndpointMode::Error => {
                return Err(EndpointError {
                    cause: EndpointCause,
                });
            }
            EndpointMode::Pending => pending::<()>().await,
        }
        self.writes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((u64::from(offset), request.into()));
        let reservation = self.gate.reserve(1).expect("one sequence");
        let observation = RegisterObservation::write(
            reservation.start_sequence(),
            AccessOrigin::Direct,
            offset,
            request,
        );
        reservation.emit(vec![observation]).expect("emit");
        Ok(())
    }
}

#[test]
fn register_access_records_literal_read_and_write_after_commit() {
    // Given a real endpoint fixture with one clone-shared observer.
    let observer = Arc::new(RecordingObserver::default());
    let endpoint = ToyEndpoint::new(
        EndpointMode::Ready,
        SharedRegisterObserver::new(observer.clone()),
    );
    let mut response = [0; 4];

    // When exact-width read and write operations commit.
    run_ready(endpoint.read(0x10, &mut response))
        .expect("read is ready")
        .expect("read succeeds");
    run_ready(endpoint.write(0x20, &[0xde, 0xad, 0xbe, 0xef]))
        .expect("write is ready")
        .expect("write succeeds");

    // Then literal bytes and the read/write request-response convention are exact.
    assert_eq!(response, [0x78, 0x56, 0x34, 0x12]);
    assert_eq!(endpoint.committed_writes(), 1);
    assert_eq!(
        *observer
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![
            RegisterObservation {
                sequence: 0,
                origin: AccessOrigin::Direct,
                kind: AccessKind::Read,
                offset: 0x10,
                width: 4,
                request_bytes: Vec::new(),
                response_bytes: vec![0x78, 0x56, 0x34, 0x12],
            },
            RegisterObservation {
                sequence: 1,
                origin: AccessOrigin::Direct,
                kind: AccessKind::Write,
                offset: 0x20,
                width: 4,
                request_bytes: vec![0xde, 0xad, 0xbe, 0xef],
                response_bytes: Vec::new(),
            },
        ]
    );
}

#[test]
fn register_access_error_preserves_source_and_reserves_nothing() {
    // Given an endpoint that rejects before commit.
    let observer = Arc::new(RecordingObserver::default());
    let endpoint = ToyEndpoint::new(
        EndpointMode::Error,
        SharedRegisterObserver::new(observer.clone()),
    );
    let mut response = [0x55; 4];

    // When the read is attempted.
    let error = run_ready(endpoint.read(0x10, &mut response))
        .expect("read is ready")
        .expect_err("read fails");

    // Then the cause, destination, sequence allocator, state, and observer remain intact.
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("literal endpoint cause")
    );
    assert_eq!(response, [0x55; 4]);
    assert_eq!(endpoint.committed_writes(), 0);
    assert_eq!(endpoint.gate.next_sequence(), Ok(0));
    assert!(
        observer
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

#[test]
fn dropping_pending_write_commits_and_reserves_nothing() {
    // Given an endpoint that deliberately remains pending before commit.
    let observer = Arc::new(RecordingObserver::default());
    let endpoint = ToyEndpoint::new(
        EndpointMode::Pending,
        SharedRegisterObserver::new(observer.clone()),
    );
    let mut future = Box::pin(endpoint.write(0x20, &[0xde, 0xad, 0xbe, 0xef]));
    let mut context = Context::from_waker(Waker::noop());

    // When the future is polled once and dropped.
    assert!(matches!(
        Future::poll(Pin::as_mut(&mut future), &mut context),
        Poll::Pending
    ));
    drop(future);

    // Then no state, reservation, or observation was committed.
    assert_eq!(endpoint.committed_writes(), 0);
    assert_eq!(endpoint.gate.next_sequence(), Ok(0));
    assert!(
        observer
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

#[test]
fn wrong_width_accesses_fail_before_mutation_or_reservation() {
    // Given ready endpoints and caller buffers with the wrong literal widths.
    let observer = Arc::new(RecordingObserver::default());
    let endpoint = ToyEndpoint::new(
        EndpointMode::Ready,
        SharedRegisterObserver::new(observer.clone()),
    );
    let mut response = [0x55; 3];

    // When the malformed exact-width operations are polled.
    let read = run_ready(endpoint.read(0x10, &mut response)).expect("read is ready");
    let write = run_ready(endpoint.write(0x20, &[0xde, 0xad, 0xbe])).expect("write is ready");

    // Then both fail without changing caller bytes, endpoint state, sequence, or transcript.
    assert!(read.is_err());
    assert!(write.is_err());
    assert_eq!(response, [0x55; 3]);
    assert_eq!(endpoint.committed_writes(), 0);
    assert_eq!(endpoint.gate.next_sequence(), Ok(0));
    assert!(
        observer
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

fn run_ready<T>(future: impl std::future::Future<Output = T>) -> Option<T> {
    let mut future = Box::pin(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(std::pin::Pin::as_mut(&mut future), &mut context) {
        std::task::Poll::Ready(value) => Some(value),
        std::task::Poll::Pending => None,
    }
}
