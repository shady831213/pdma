//! Snapshot lifetime and ownership through the shared fan lifecycle.

mod arbitration_snapshots {
    mod support;

    use hardware_interface_pipeline::{
        Arbitrate, Arbitration, BranchError, FanIn, FanPlan, FanPolicy, StepOutcome,
    };
    use std::{cell::Cell, rc::Rc};

    use self::support::{Context, Hardware, Output, Sample, Select, Source, finish, inputs};

    #[test]
    fn snapshots_forward_only_winner_and_drop_other_owned_values_once() {
        // Given: three owned samples, with the winner away from the first input.
        let drops = Rc::new(Cell::new(0));
        let mut context = Context {
            ready: vec![false, true, true],
            ..Context::default()
        };
        let mut hardware = Hardware::new(
            vec![Source::default(), Source::default(), Source::default()],
            Arbitrate::snapshots(Select(Rc::default())),
            Output,
        );
        let mut state =
            hardware_interface_pipeline::Operation::new(inputs(&drops, context.ready.len()));
        // When: the sample round routes its winner before stepping the output.
        for _ in 0..2 {
            assert!(matches!(
                state.step(&mut hardware, &mut context),
                Ok(StepOutcome::InProgress)
            ));
        }
        // Then: stale snapshots are dropped, and only the winner becomes a receipt.
        assert_eq!(drops.get(), 2);
        assert!(context.effects.is_empty());
        let receipts = finish(&mut hardware, &mut state, &mut context)
            .unwrap()
            .unwrap();
        assert_eq!(context.effects, [1]);
        assert_eq!(
            receipts
                .iter()
                .map(|(_, value)| value.id)
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(drops.get(), 2);
        drop(receipts);
        assert_eq!(drops.get(), 3);
    }

    #[test]
    fn snapshots_without_a_winner_complete_without_output_effects() {
        // Given: either an empty population or completed but ineligible samples.
        for count in [0, 3] {
            let drops = Rc::new(Cell::new(0));
            let mut context = Context::default();
            let mut hardware = Hardware::new(
                (0..count).map(|_| Source::default()).collect(),
                Arbitrate::snapshots(Select(Rc::default())),
                Output,
            );
            let mut state = hardware_interface_pipeline::Operation::new(inputs(&drops, count));
            // When: the round finishes with no selection.
            let receipts = finish(&mut hardware, &mut state, &mut context)
                .unwrap()
                .unwrap();
            // Then: every sample retires without starting the output.
            assert!(receipts.is_empty());
            assert!(context.effects.is_empty());
            assert_eq!(drops.get(), count);
        }
    }

    #[test]
    fn snapshots_wait_for_unfinished_input_before_calling_selector() {
        // Given: one ready sample and one source held unfinished.
        let calls = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        let mut context = Context {
            ready: vec![true, true],
            ..Context::default()
        };
        let mut hardware = Hardware::new(
            vec![Source::default(), Source::default()],
            Arbitrate::snapshots(Select(Rc::clone(&calls))),
            Output,
        );
        let mut samples = inputs(&drops, 2);
        samples.get_mut(1).unwrap().1 = true;
        let mut state = hardware_interface_pipeline::Operation::new(samples);
        // When: hardware progresses while a source is held, then releases it.
        for _ in 0..9 {
            assert!(matches!(
                state.step(&mut hardware, &mut context),
                Ok(StepOutcome::InProgress)
            ));
        }
        assert_eq!(calls.get(), 0);
        assert_eq!(drops.get(), 0);
        assert!(context.effects.is_empty());
        context.release = true;
        let receipts = finish(&mut hardware, &mut state, &mut context)
            .unwrap()
            .unwrap();
        // Then: the selector runs once on the complete population.
        assert_eq!(calls.get(), 1);
        assert_eq!(context.effects, [0]);
        drop(receipts);
        assert_eq!(drops.get(), 2);
    }

    struct Invalid;
    impl Arbitration<Sample, ()> for Invalid {
        fn select(&mut self, values: &[Option<Sample>]) -> Result<Option<usize>, ()> {
            Ok(Some(values.len()))
        }
    }
    #[test]
    fn invalid_snapshot_winner_fails_before_output_effects() {
        // Given: a selector returns an index outside its complete sample population.
        let drops = Rc::new(Cell::new(0));
        let mut context = Context::default();
        let mut hardware = FanIn::new(
            vec![Source::default()],
            Arbitrate::snapshots(Invalid),
            Output,
        );
        let mut state = hardware_interface_pipeline::Operation::new(inputs(&drops, 1));
        // When: Fan validates the snapshot plan.
        assert!(matches!(
            state.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        ));
        let result = state.step(&mut hardware, &mut context);
        // Then: invalid selection terminates without any output effect or leaked sample.
        assert!(matches!(
            result,
            Ok(StepOutcome::Complete(Err(BranchError::InvalidIndex {
                index: 1,
                ports: 1
            })))
        ));
        assert!(context.effects.is_empty());
        assert!(state.is_complete());
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn repeated_snapshot_rounds_observe_changed_readiness() {
        // Given: one reusable fan and readiness that changes between invocations.
        let drops = Rc::new(Cell::new(0));
        let mut context = Context::default();
        let mut hardware = Hardware::new(
            vec![Source::default(), Source::default()],
            Arbitrate::snapshots(Select(Rc::default())),
            Output,
        );
        // When: each round samples a different ready source.
        for ready in [vec![true, false], vec![false, true], vec![false, false]] {
            context.ready = ready;
            let mut state = hardware_interface_pipeline::Operation::new(inputs(&drops, 2));
            drop(
                finish(&mut hardware, &mut state, &mut context)
                    .unwrap()
                    .unwrap(),
            );
        }
        // Then: stale eligibility never runs and the idle round emits nothing.
        assert_eq!(context.effects, [0, 1]);
        assert_eq!(drops.get(), 6);
    }

    #[test]
    fn busy_output_defers_snapshot_selection() {
        // Given: complete samples with no available destination.
        let calls = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        let values: Vec<_> = inputs(&drops, 2)
            .into_iter()
            .map(|(value, _)| Some(value))
            .collect();
        let mut policy = Arbitrate::snapshots(Select(Rc::clone(&calls)));
        // When: planning observes destination backpressure.
        let plan = policy.plan(&values, &[false]).unwrap();
        // Then: no samples are consumed and no arbitration state advances.
        assert_eq!(plan, FanPlan::default());
        assert_eq!(calls.get(), 0);
        assert_eq!(drops.get(), 0);
    }
}
