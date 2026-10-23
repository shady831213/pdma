use super::*;
use std::{sync::mpsc, time::Duration};

#[derive(Default)]
struct RecursiveObserver {
    delivered: Mutex<Vec<u64>>,
    nested: Mutex<Option<BlockingObservationReservation>>,
    result: Mutex<Option<Result<(), ObservationGateError>>>,
}

impl crate::RegisterObserver for RecursiveObserver {
    fn observe(&self, observation: RegisterObservation) {
        self.delivered
            .lock()
            .expect("delivered lock")
            .push(observation.sequence);
        let nested = self.nested.lock().expect("nested lock").take();
        if let Some(nested) = nested {
            assert_eq!(
                nested.backend.next_sequence(),
                Ok(nested.start_sequence() + 2)
            );
            let sequence = nested.start_sequence();
            let result = nested.emit(vec![RegisterObservation::write(
                sequence,
                AccessOrigin::Direct,
                0x20,
                &[0xbb],
            )]);
            *self.result.lock().expect("result lock") = Some(result);
        }
    }
}

#[test]
fn recursive_emission_faults_and_wakes_waiters() {
    check_recursive_emission(1);
}

#[test]
fn recursive_emission_stops_remaining_batch_callbacks() {
    check_recursive_emission(2);
}

fn check_recursive_emission(count: usize) {
    let observer = Arc::new(RecursiveObserver::default());
    let gate = BlockingObservationBackend::new(SharedRegisterObserver::new(observer.clone()));
    let outer = gate.reserve(count).expect("outer reservation");
    *observer.nested.lock().expect("nested lock") = Some(gate.reserve(1).expect("nested"));
    let follower = gate.reserve(1).expect("follower reservation");
    let follower_sequence = follower.start_sequence();
    let follower_thread = thread::spawn(move || {
        follower.emit(vec![RegisterObservation::write(
            follower_sequence,
            AccessOrigin::Direct,
            0x30,
            &[0xcc],
        )])
    });
    wait_for_one_waiter(&gate);
    let (sender, receiver) = mpsc::channel();
    let outer_thread = thread::spawn(move || {
        let observations = (0..count)
            .map(|index| {
                RegisterObservation::write(
                    u64::try_from(index).expect("sequence fits"),
                    AccessOrigin::Batch,
                    0x10,
                    &[0xaa],
                )
            })
            .collect();
        let result = outer.emit(observations);
        sender.send(result).expect("report outer result");
    });
    let completed = receiver.recv_timeout(Duration::from_secs(2));
    if completed.is_err() {
        gate.fault(GateFault::InvalidEmission);
    }
    outer_thread.join().expect("outer joined after cleanup");
    let follower_result = follower_thread
        .join()
        .expect("follower joined after cleanup");
    assert!(
        completed.is_ok(),
        "recursive emission timed out; workers cleaned up"
    );
    let expected = Err(ObservationGateError::Faulted {
        reason: GateFault::InvalidEmission,
    });
    assert_eq!(completed.expect("completed"), expected);
    assert_eq!(follower_result, expected);
    assert_eq!(
        *observer.result.lock().expect("result lock"),
        Some(expected)
    );
    assert_eq!(gate.next_sequence(), expected.map(|()| 0));
    assert_eq!(*observer.delivered.lock().expect("delivered lock"), [0]);
}
