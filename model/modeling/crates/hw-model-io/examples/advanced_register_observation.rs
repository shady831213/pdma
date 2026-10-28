//! Advanced register transport with ordered observations of committed accesses.
//!
//! Run: `cargo run -p hw-model-io --example advanced_register_observation`.
//! The endpoint supplies access semantics; `ObservationGate` orders its receipts.
//! Success, rejection, and cancellation before commit produce different traces.
//! Manual polls make those boundaries visible without a scheduler or sleeps.

use hw_model_io::register::RegisterAccess;
use hw_model_io::register::observation::{
    AccessKind, AccessOrigin, ObservationGate, ObservationGateError, RegisterObservation,
    RegisterObserver, SharedRegisterObserver,
};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::{Future, pending};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Ready,
    Reject,
    Pending,
}

#[derive(Debug)]
struct EndpointCause;

impl Display for EndpointCause {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("adverse endpoint cause")
    }
}

impl Error for EndpointCause {}

#[derive(Debug)]
enum EndpointError {
    Rejected(EndpointCause),
    WrongWidth(usize),
    Gate(ObservationGateError),
}

impl Display for EndpointError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(_) => formatter.write_str("typed adverse endpoint error"),
            Self::WrongWidth(width) => write!(formatter, "wrong register width {width}"),
            Self::Gate(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for EndpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rejected(cause) => Some(cause),
            Self::Gate(error) => Some(error),
            Self::WrongWidth(_) => None,
        }
    }
}

#[derive(Debug, Default)]
struct EndpointStats {
    calls: usize,
    committed_writes: usize,
}

#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<RegisterObservation>>);

impl Recorder {
    fn snapshot(&self) -> Vec<RegisterObservation> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl RegisterObserver for Recorder {
    fn observe(&self, observation: RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

#[derive(Debug)]
struct Endpoint {
    mode: Mode,
    gate: ObservationGate,
    stats: Mutex<EndpointStats>,
}

impl Endpoint {
    fn new(mode: Mode, recorder: &Arc<Recorder>) -> Self {
        Self {
            mode,
            gate: ObservationGate::new(SharedRegisterObserver::new(recorder.clone())),
            stats: Mutex::new(EndpointStats::default()),
        }
    }

    fn counts(&self) -> (usize, usize) {
        let stats = self
            .stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (stats.calls, stats.committed_writes)
    }
}

impl RegisterAccess for Endpoint {
    type Error = EndpointError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .calls += 1;
        if response.len() != 4 {
            return Err(EndpointError::WrongWidth(response.len()));
        }
        if self.mode == Mode::Reject {
            return Err(EndpointError::Rejected(EndpointCause));
        }
        response.copy_from_slice(&[0x78, 0x56, 0x34, 0x12]);
        let reservation = self.gate.reserve(1).map_err(EndpointError::Gate)?;
        let observation = RegisterObservation::read(
            reservation.start_sequence(),
            AccessOrigin::Direct,
            offset,
            response,
        );
        reservation
            .emit(vec![observation])
            .map_err(EndpointError::Gate)
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .calls += 1;
        if request.len() != 4 {
            return Err(EndpointError::WrongWidth(request.len()));
        }
        match self.mode {
            Mode::Ready => {}
            Mode::Reject => return Err(EndpointError::Rejected(EndpointCause)),
            Mode::Pending => return pending::<Result<(), EndpointError>>().await,
        }
        // Reserve receipt order before committing. Neither a rejected request nor
        // this endpoint's pending request reaches the commit or consumes a sequence.
        let mut stats = self
            .stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reservation = self.gate.reserve(1).map_err(EndpointError::Gate)?;
        stats.committed_writes += 1;
        drop(stats);
        let observation = RegisterObservation::write(
            reservation.start_sequence(),
            AccessOrigin::Direct,
            offset,
            request,
        );
        reservation
            .emit(vec![observation])
            .map_err(EndpointError::Gate)
    }
}

fn poll_once<T>(future: Pin<&mut impl Future<Output = T>>) -> Poll<T> {
    Future::poll(future, &mut Context::from_waker(Waker::noop()))
}

fn ready<T>(future: impl Future<Output = T>) -> Option<T> {
    let mut future = Box::pin(future);
    match poll_once(Pin::as_mut(&mut future)) {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let happy_recorder = Arc::new(Recorder::default());
    let happy = Endpoint::new(Mode::Ready, &happy_recorder);
    let mut response = [0; 4];
    ready(happy.read(0x10, &mut response)).ok_or("happy read unexpectedly pending")??;
    ready(happy.write(0x20, &[0xde, 0xad, 0xbe, 0xef]))
        .ok_or("happy write unexpectedly pending")??;
    let transcript = happy_recorder.snapshot();
    let expected = vec![
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
    ];
    if transcript != expected || happy.counts() != (2, 1) {
        return Err("happy instrumentation mismatch".into());
    }
    println!("sequence=0 direct read offset=0x10 width=4 response={response:02x?}");
    println!("sequence=1 direct write offset=0x20 width=4 request=[de, ad, be, ef]");

    // Typed errors retain their endpoint cause and produce no success receipt.
    let error_recorder = Arc::new(Recorder::default());
    let rejected = Endpoint::new(Mode::Reject, &error_recorder);
    let mut rejected_response = [0x55; 4];
    let error = ready(rejected.read(0x10, &mut rejected_response))
        .ok_or("adverse read unexpectedly pending")?
        .err()
        .ok_or("adverse read unexpectedly succeeded")?;
    println!(
        "adverse error={error} source={} calls={} committed_state={} reserved_sequences={} observations={}",
        error.source().ok_or("missing endpoint source")?,
        rejected.counts().0,
        rejected.counts().1,
        rejected.gate.next_sequence()?,
        error_recorder.snapshot().len(),
    );

    // This endpoint suspends before effects. Dropping its future therefore leaves
    // no committed write; RegisterAccess itself does not promise generic rollback.
    let pending_recorder = Arc::new(Recorder::default());
    let pending_endpoint = Endpoint::new(Mode::Pending, &pending_recorder);
    let request = [0xde, 0xad, 0xbe, 0xef];
    let mut pending_write = Box::pin(pending_endpoint.write(0x20, &request));
    if !matches!(poll_once(Pin::as_mut(&mut pending_write)), Poll::Pending) {
        return Err("pending write unexpectedly completed".into());
    }
    drop(pending_write);
    println!(
        "dropped pending write calls={} committed_state={} reserved_sequences={} observations={}",
        pending_endpoint.counts().0,
        pending_endpoint.counts().1,
        pending_endpoint.gate.next_sequence()?,
        pending_recorder.snapshot().len(),
    );
    Ok(())
}
