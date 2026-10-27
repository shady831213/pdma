#![allow(missing_docs, reason = "external fixture test")]

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hardware_interface_io::register::observation::{
    AccessOrigin, LocalObservationGate, ObservationGate, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};

#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<RegisterObservation>>);

impl RegisterObserver for Recorder {
    fn observe(&self, observation: RegisterObservation) {
        lock(&self.0).push(observation);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn local_observations() -> Result<Vec<RegisterObservation>, Box<dyn std::error::Error>> {
    let recorder = Arc::new(Recorder::default());
    let backend = LocalObservationGate::new(SharedRegisterObserver::new(recorder.clone()));
    backend.reserve(2)?.emit(vec![
        RegisterObservation::write(0, AccessOrigin::Direct, 8, &[0x0b, 0xa1]),
        RegisterObservation::read(1, AccessOrigin::Direct, 8, &[0x07, 0xb2]),
    ])?;
    let transcript = lock(&recorder.0).clone();
    Ok(transcript)
}

fn blocking_observations() -> Result<Vec<RegisterObservation>, Box<dyn std::error::Error>> {
    let recorder = Arc::new(Recorder::default());
    let backend = ObservationGate::new(SharedRegisterObserver::new(recorder.clone()));
    backend.reserve(2)?.emit(vec![
        RegisterObservation::write(0, AccessOrigin::Direct, 8, &[0x0b, 0xa1]),
        RegisterObservation::read(1, AccessOrigin::Direct, 8, &[0x07, 0xb2]),
    ])?;
    let transcript = lock(&recorder.0).clone();
    Ok(transcript)
}

#[test]
fn observation_policies_preserve_ordered_access_receipts() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: the same committed accesses submitted to local and blocking policies.
    let expected = [
        RegisterObservation::write(0, AccessOrigin::Direct, 8, &[0x0b, 0xa1]),
        RegisterObservation::read(1, AccessOrigin::Direct, 8, &[0x07, 0xb2]),
    ];
    // When: both policies reserve and deliver the access receipts.
    let local = local_observations()?;
    let blocking = blocking_observations()?;
    // Then: each observer sees all receipts exactly once, in reservation order.
    assert_eq!(local, expected);
    assert_eq!(blocking, expected);
    println!("observation_local=2 observation_blocking=2 transcript_equal=true");
    Ok(())
}
