use super::error::{ConsumerError, lock};
use hir::{
    AccessKind, AccessOrigin, ObservationGate, RegisterAccess, RegisterObservation,
    RegisterObserver, SharedRegisterObserver, TraceBuffer,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Default)]
struct RegisterState {
    pub calls: usize,
    pub commits: usize,
    pub write_bytes: Vec<u8>,
    pub read_bytes: Vec<u8>,
    storage: BTreeMap<u32, [u8; 4]>,
}

#[derive(Debug)]
struct RegisterRecorder {
    observations: Mutex<Vec<RegisterObservation>>,
    trace: TraceBuffer<(usize, &'static str)>,
}

impl RegisterRecorder {
    const fn new(trace: TraceBuffer<(usize, &'static str)>) -> Self {
        Self {
            observations: Mutex::new(Vec::new()),
            trace,
        }
    }

    fn observations(&self) -> Vec<RegisterObservation> {
        lock(&self.observations).clone()
    }
}

impl RegisterObserver for RegisterRecorder {
    fn observe(&self, observation: RegisterObservation) {
        let label = match observation.kind {
            AccessKind::Read => "register-read",
            AccessKind::Write => "register-write",
        };
        self.trace.push_with(|sequence| (sequence, label));
        lock(&self.observations).push(observation);
    }
}

#[derive(Debug)]
struct RegisterEndpoint {
    gate: ObservationGate,
    state: Arc<Mutex<RegisterState>>,
}

impl RegisterEndpoint {
    fn new(recorder: Arc<RegisterRecorder>, state: Arc<Mutex<RegisterState>>) -> Self {
        lock(&state).storage.insert(0x10, [0x78, 0x56, 0x34, 0x12]);
        Self {
            gate: ObservationGate::new(SharedRegisterObserver::new(recorder)),
            state,
        }
    }
}

impl RegisterAccess for RegisterEndpoint {
    type Error = ConsumerError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        if offset != 0x10 || response.len() != 4 {
            return Err(ConsumerError::RegisterShape);
        }
        let reservation = self.gate.reserve(1).map_err(ConsumerError::Gate)?;
        let sequence = reservation.start_sequence();
        {
            let mut state = lock(&self.state);
            let value = state
                .storage
                .get(&offset)
                .ok_or(ConsumerError::RegisterShape)?;
            response.copy_from_slice(value);
            state.calls = state.calls.saturating_add(1);
            state.commits = state.commits.saturating_add(1);
            state.read_bytes = response.to_vec();
        }
        reservation
            .emit(vec![RegisterObservation::read(
                sequence,
                AccessOrigin::Direct,
                offset,
                response,
            )])
            .map_err(ConsumerError::Gate)
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        if offset != 0x20 || request.len() != 4 {
            return Err(ConsumerError::RegisterShape);
        }
        let reservation = self.gate.reserve(1).map_err(ConsumerError::Gate)?;
        let sequence = reservation.start_sequence();
        {
            let mut state = lock(&self.state);
            let value: [u8; 4] = request
                .try_into()
                .map_err(|_| ConsumerError::RegisterShape)?;
            state.storage.insert(offset, value);
            state.calls = state.calls.saturating_add(1);
            state.commits = state.commits.saturating_add(1);
            state.write_bytes = request.to_vec();
        }
        reservation
            .emit(vec![RegisterObservation::write(
                sequence,
                AccessOrigin::Direct,
                offset,
                request,
            )])
            .map_err(ConsumerError::Gate)
    }
}

pub(super) async fn run_register(
    trace: &TraceBuffer<(usize, &'static str)>,
) -> Result<(), ConsumerError> {
    let state = Arc::new(Mutex::new(RegisterState::default()));
    let recorder = Arc::new(RegisterRecorder::new(trace.clone()));
    let endpoint = RegisterEndpoint::new(Arc::clone(&recorder), Arc::clone(&state));
    endpoint.write(0x20, &[0xde, 0xad, 0xbe, 0xef]).await?;
    let mut response = [0_u8; 4];
    endpoint.read(0x10, &mut response).await?;
    let stored = lock(&state);
    assert_state(&stored, response)?;
    let expected = [
        RegisterObservation::write(0, AccessOrigin::Direct, 0x20, &[0xde, 0xad, 0xbe, 0xef]),
        RegisterObservation::read(1, AccessOrigin::Direct, 0x10, &[0x78, 0x56, 0x34, 0x12]),
    ];
    let actual = recorder.observations();
    drop(stored);
    ensure(
        actual == expected,
        "register observation transcript changed",
    )
}

fn assert_state(state: &RegisterState, response: [u8; 4]) -> Result<(), ConsumerError> {
    ensure(
        state.calls == 2 && state.commits == 2,
        "register calls were not committed",
    )?;
    ensure(
        state.write_bytes == [0xde, 0xad, 0xbe, 0xef],
        "register write bytes changed",
    )?;
    ensure(
        state.read_bytes == [0x78, 0x56, 0x34, 0x12],
        "register storage read changed",
    )?;
    ensure(
        response == [0x78, 0x56, 0x34, 0x12],
        "register response changed",
    )
}

const fn ensure(condition: bool, message: &'static str) -> Result<(), ConsumerError> {
    if condition {
        Ok(())
    } else {
        Err(ConsumerError::Invariant(message))
    }
}
