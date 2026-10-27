use super::*;

#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<RegisterObservation>>);

impl crate::RegisterObserver for Recorder {
    fn observe(&self, observation: RegisterObservation) {
        self.0.lock().expect("recorder lock").push(observation);
    }
}

#[test]
#[allow(clippy::panic, reason = "mutex poisoning is the behavior under test")]
fn synchronization_poison_terminally_faults_gate() {
    let gate = BlockingObservationBackend::new(SharedRegisterObserver::new(Arc::new(
        NoopRegisterObserver,
    )));
    let poison_gate = gate.clone();

    let panic_result = std::panic::catch_unwind(AssertUnwindSafe(move || {
        let _guard = poison_gate.state.lock().expect("initial lock");
        panic!("deliberate gate poison");
    }));

    assert!(panic_result.is_err());
    assert_eq!(
        gate.reserve(1).expect_err("poison rejected"),
        ObservationGateError::Faulted {
            reason: GateFault::SynchronizationPoisoned,
        }
    );
    assert_eq!(
        gate.reserve(1).expect_err("terminal fault retained"),
        ObservationGateError::Faulted {
            reason: GateFault::SynchronizationPoisoned,
        }
    );
}

#[test]
fn dropped_reservation_wakes_an_already_blocked_follower() {
    enum Abandon {
        Direct,
        Inspect,
        Mutate,
        Unwind,
    }
    for action in [
        Abandon::Direct,
        Abandon::Inspect,
        Abandon::Mutate,
        Abandon::Unwind,
    ] {
        let gate = BlockingObservationBackend::new(SharedRegisterObserver::new(Arc::new(
            NoopRegisterObserver,
        )));
        let earlier = gate.reserve(1).expect("earlier reservation");
        let follower = gate.reserve(1).expect("follower reservation");
        let follower_thread = thread::spawn(move || {
            follower.emit(vec![RegisterObservation::write(
                1,
                crate::AccessOrigin::Direct,
                0x20,
                &[0xbb],
            )])
        });
        wait_for_one_waiter(&gate);

        match action {
            Abandon::Direct => drop(earlier),
            Abandon::Inspect => crate::ObservationBackend::inspect(&gate, |_| drop(earlier))
                .expect("inspection returns"),
            Abandon::Mutate => crate::ObservationBackend::mutate(&gate, |_| drop(earlier))
                .expect("mutation returns"),
            Abandon::Unwind => {
                assert!(
                    std::panic::catch_unwind(AssertUnwindSafe(|| {
                        crate::ObservationBackend::inspect(&gate, |_| {
                            drop(earlier);
                            std::panic::resume_unwind(Box::new("interrupted inspection"));
                        })
                    }))
                    .is_err()
                );
            }
        }

        assert_eq!(
            follower_thread.join().expect("follower joins"),
            Err(ObservationGateError::Faulted {
                reason: GateFault::ReservationDropped,
            })
        );
    }
}

#[test]
fn later_emitter_is_waiting_before_earlier_range_emits() {
    let recorder = Arc::new(Recorder::default());
    let gate = BlockingObservationBackend::new(SharedRegisterObserver::new(recorder.clone()));
    let earlier = gate.reserve(1).expect("earlier reservation");
    let later = gate.reserve(1).expect("later reservation");
    let later_thread = thread::spawn(move || {
        later.emit(vec![RegisterObservation::write(
            1,
            crate::AccessOrigin::Direct,
            0x20,
            &[0xbb],
        )])
    });
    wait_for_one_waiter(&gate);

    earlier
        .emit(vec![RegisterObservation::write(
            0,
            crate::AccessOrigin::Direct,
            0x10,
            &[0xaa],
        )])
        .expect("earlier emits");
    later_thread
        .join()
        .expect("later joins")
        .expect("later emits");

    let sequences = recorder
        .0
        .lock()
        .expect("recorder lock")
        .iter()
        .map(|observation| observation.sequence)
        .collect::<Vec<_>>();
    assert_eq!(sequences, [0, 1]);
}

#[test]
fn direct_emitter_is_waiting_before_batch_range_emits_contiguously() {
    let recorder = Arc::new(Recorder::default());
    let gate = BlockingObservationBackend::new(SharedRegisterObserver::new(recorder.clone()));
    let batch = gate.reserve(3).expect("batch reservation");
    let direct = gate.reserve(1).expect("direct reservation");
    let direct_thread = thread::spawn(move || {
        direct.emit(vec![RegisterObservation::write(
            3,
            crate::AccessOrigin::Direct,
            0x40,
            &[0xdd],
        )])
    });
    wait_for_one_waiter(&gate);

    batch
        .emit(vec![
            RegisterObservation::write(0, crate::AccessOrigin::Batch, 0x10, &[0xa0]),
            RegisterObservation::write(1, crate::AccessOrigin::Batch, 0x20, &[0xb0]),
            RegisterObservation::write(2, crate::AccessOrigin::Batch, 0x30, &[0xc0]),
        ])
        .expect("batch emits");
    direct_thread
        .join()
        .expect("direct joins")
        .expect("direct emits");

    let transcript = recorder.0.lock().expect("recorder lock").clone();
    assert_eq!(
        transcript
            .iter()
            .map(|observation| (observation.sequence, observation.origin))
            .collect::<Vec<_>>(),
        [
            (0, crate::AccessOrigin::Batch),
            (1, crate::AccessOrigin::Batch),
            (2, crate::AccessOrigin::Batch),
            (3, crate::AccessOrigin::Direct),
        ]
    );
}
