use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

/// Whether a committed access originated directly or within a closed batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessOrigin {
    /// One direct register operation.
    Direct,
    /// One item in a committed batch range.
    Batch,
}

/// Direction of one committed register access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    /// Register read.
    Read,
    /// Register write.
    Write,
}

/// Owned durable description of one committed raw register access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterObservation {
    /// Global commit sequence.
    pub sequence: u64,
    /// Direct or batch origin.
    pub origin: AccessOrigin,
    /// Read or write direction.
    pub kind: AccessKind,
    /// Numeric register offset.
    pub offset: u32,
    /// Exact access width in bytes.
    pub width: usize,
    /// Exact write bytes; empty for reads.
    pub request_bytes: Vec<u8>,
    /// Exact read bytes; empty for writes.
    pub response_bytes: Vec<u8>,
}

impl RegisterObservation {
    /// Builds an owned successful-read observation.
    pub fn read(sequence: u64, origin: AccessOrigin, offset: u32, response: &[u8]) -> Self {
        Self {
            sequence,
            origin,
            kind: AccessKind::Read,
            offset,
            width: response.len(),
            request_bytes: Vec::new(),
            response_bytes: response.to_vec(),
        }
    }

    /// Builds an owned successful-write observation.
    pub fn write(sequence: u64, origin: AccessOrigin, offset: u32, request: &[u8]) -> Self {
        Self {
            sequence,
            origin,
            kind: AccessKind::Write,
            offset,
            width: request.len(),
            request_bytes: request.to_vec(),
            response_bytes: Vec::new(),
        }
    }
}

/// Synchronous infallible sink for committed register observations.
///
/// Callbacks must not wait for later observations from the same gate, including
/// through another thread: ordered delivery cannot advance until this call returns.
/// A blocking gate detects same-thread recursive emission and terminally rejects it;
/// querying the gate or reserving work for later emission is still permitted.
///
/// A fallible callback cannot implement this interface:
/// ```compile_fail
/// use hardware_interface_runtime::{RegisterObservation, RegisterObserver};
/// struct Fallible;
/// impl RegisterObserver for Fallible {
///     fn observe(&self, _: RegisterObservation) -> Result<(), ()> { Ok(()) }
/// }
/// ```
/// An asynchronous callback cannot implement it either:
/// ```compile_fail
/// use hardware_interface_runtime::{RegisterObservation, RegisterObserver};
/// struct Asynchronous;
/// impl RegisterObserver for Asynchronous {
///     async fn observe(&self, _: RegisterObservation) {}
/// }
/// ```
pub trait RegisterObserver: Send + Sync {
    /// Receives one committed observation before its caller returns.
    fn observe(&self, observation: RegisterObservation);
}

/// Clone-shared object-safe register observer.
#[derive(Clone)]
pub struct SharedRegisterObserver(Arc<dyn RegisterObserver>);

impl SharedRegisterObserver {
    /// Erases and shares an observer implementation.
    pub fn new(observer: Arc<dyn RegisterObserver>) -> Self {
        Self(observer)
    }

    pub(crate) fn observe(&self, observation: RegisterObservation) {
        self.0.observe(observation);
    }
}

impl Debug for SharedRegisterObserver {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("SharedRegisterObserver(..)")
    }
}

/// Observer that intentionally discards every committed observation.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopRegisterObserver;

impl RegisterObserver for NoopRegisterObserver {
    fn observe(&self, _observation: RegisterObservation) {}
}
