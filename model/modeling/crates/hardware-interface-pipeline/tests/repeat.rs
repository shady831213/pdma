//! Behavioral contracts for feedback through persistent hardware stages.

mod repeat {
    mod stages;

    use hardware_interface_pipeline::{Operation, Repeat, RunOutcome, Stage, StepOutcome, Then};

    use stages::{Finish, Next, Visit};

    #[test]
    fn feedback_composes_with_then_and_retains_pending_state() {
        // Given: each visit suspends after recording its accepted input.
        let mut hardware = Repeat::new(Then::new(Visit { calls: 0 }, Next::new(Finish::At(2))));
        let mut operation = Operation::new(0);
        let mut visits = Vec::new();

        // When: the composed operation advances across all three iterations.
        let outcomes: Vec<_> = (0..9)
            .map(|_| operation.step(&mut hardware, &mut visits))
            .collect();

        // Then: each pending input is retained and feedback starts at the next boundary.
        assert_eq!(
            outcomes,
            [
                Ok(RunOutcome::Pending("accepted 0".to_owned())),
                Ok(RunOutcome::Progressed),
                Ok(RunOutcome::Progressed),
                Ok(RunOutcome::Pending("accepted 1".to_owned())),
                Ok(RunOutcome::Progressed),
                Ok(RunOutcome::Progressed),
                Ok(RunOutcome::Pending("accepted 2".to_owned())),
                Ok(RunOutcome::Progressed),
                Ok(RunOutcome::Complete(Ok("finished 2".to_owned()))),
            ]
        );
        assert_eq!(visits, [0, 1, 2]);
        assert_eq!(hardware.stage.upstream.calls, 6);
        assert_eq!(hardware.stage.downstream.calls, 3);
    }

    #[test]
    fn feedback_performs_at_most_one_child_step_per_outer_step() {
        // Given: an immediately ready child requests another iteration.
        let mut hardware = Repeat::new(Next::new(Finish::At(2)));
        let state = Repeat::<Next>::start(0);
        let mut context = Vec::new();

        // When: the caller advances only one boundary.
        let outcome = hardware.step(&mut context, state);

        // Then: the next child is initialized but has not interacted.
        assert!(matches!(outcome, StepOutcome::Progressed(_)));
        assert_eq!(hardware.stage.calls, 1);
    }

    #[test]
    fn completion_error_and_missing_child_work_are_terminal_at_operation_boundary() {
        // Given: each terminal child outcome retained by an Operation.
        for (finish, expected) in [
            (
                Finish::At(0),
                Ok(RunOutcome::Complete(Ok("finished 0".to_owned()))),
            ),
            (
                Finish::Fail,
                Ok(RunOutcome::Complete(Err("child failure".to_owned()))),
            ),
            (
                Finish::Missing,
                Ok(RunOutcome::Complete(Err("missing child work".to_owned()))),
            ),
        ] {
            let mut hardware = Repeat::new(Next::new(finish));
            let mut state = Operation::new(0);
            let mut context = Vec::new();

            // When: the terminal result is observed and the operation is stepped twice more.
            let outcomes = [
                state.step(&mut hardware, &mut context),
                state.step(&mut hardware, &mut context),
                state.step(&mut hardware, &mut context),
            ];

            // Then: the child runs once and its terminal value is forwarded only once.
            assert_eq!(
                outcomes,
                [
                    expected,
                    Err(hardware_interface_pipeline::AlreadyComplete),
                    Err(hardware_interface_pipeline::AlreadyComplete)
                ]
            );
            assert_eq!(hardware.stage.calls, 1);
        }
    }

    #[test]
    fn fresh_operations_reuse_hardware_after_feedback_completes() {
        // Given: one persistent child with a terminal input of two.
        let mut hardware = Repeat::new(Next::new(Finish::At(2)));
        let mut context = Vec::new();
        let mut first = Operation::new(1);
        let mut second = Operation::new(2);

        // When: independent operations use the same hardware successively.
        let outcomes = [
            first.step(&mut hardware, &mut context),
            first.step(&mut hardware, &mut context),
            second.step(&mut hardware, &mut context),
        ];

        // Then: the second operation completes using fresh state and persistent hardware.
        assert_eq!(
            outcomes,
            [
                Ok(RunOutcome::Progressed),
                Ok(RunOutcome::Complete(Ok("finished 2".to_owned()))),
                Ok(RunOutcome::Complete(Ok("finished 2".to_owned()))),
            ]
        );
        assert_eq!(hardware.stage.calls, 3);
    }
}
