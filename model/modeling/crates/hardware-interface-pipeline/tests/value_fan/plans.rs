use crate::{Context, Sink, Source};
use hardware_interface_pipeline::{BranchError, Fan, FanPlan, FanPolicy, Stage, StepOutcome};

struct SplitJoin(usize);
impl FanPolicy<String, String, &'static str> for SplitJoin {
    fn plan(
        &mut self,
        values: &[Option<String>],
        available: &[bool],
    ) -> Result<FanPlan, &'static str> {
        if values.iter().all(Option::is_some) && available.iter().all(|v| *v) {
            Ok(FanPlan {
                inputs: vec![0, 1],
                outputs: vec![0, 1],
            })
        } else {
            Ok(FanPlan::default())
        }
    }
    fn route(&mut self, values: Vec<String>) -> Result<Vec<String>, &'static str> {
        let mut joined: String = values.into_iter().collect();
        let tail = joined.split_off(self.0);
        Ok(vec![joined, tail])
    }
}

#[test]
fn full_fan_transforms_two_completed_values_into_two_owned_outputs() {
    // Given: two sources and two outputs connected through a join-and-split policy.
    let first = "ab".to_owned();
    let second = "cdef".to_owned();
    let combined = format!("{first}{second}");
    let split_at = 3;
    let (head, tail) = combined.split_at(split_at);
    let expected = vec![(0, head.len()), (1, tail.len())];
    let mut hardware = Fan::new(
        vec![Source(0), Source(1)],
        SplitJoin(split_at),
        vec![Sink, Sink],
    );
    let mut state = <Fan<Source, SplitJoin, Sink> as Stage<Context>>::start(vec![first, second]);
    let mut context = Context {
        released: true,
        ..Context::default()
    };
    // When: value collection, routing, and incomplete output work cross separate boundaries.
    for _ in 0..5 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    let result = hardware.step(&mut context, &mut state);
    // Then: the actual N-to-M composition returns both outputs, with no duplicate input reads.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(expected))));
    assert_eq!(context.writes, [head, tail]);
    assert_eq!(context.reads, [0, 1]);
}

enum Invalid {
    EmptyInput,
    MissingValue,
    BusyOutput,
    WrongCount,
    RejectAfterFirst,
}
impl FanPolicy<String, String, &'static str> for Invalid {
    fn plan(
        &mut self,
        values: &[Option<String>],
        available: &[bool],
    ) -> Result<FanPlan, &'static str> {
        if matches!(self, Self::RejectAfterFirst) {
            if available.first() != Some(&true) {
                return Ok(FanPlan::default());
            }
            if values.first().is_some_and(Option::is_none) {
                return Err("later plan failed");
            }
        }
        Ok(match self {
            Self::EmptyInput => FanPlan {
                inputs: vec![],
                outputs: vec![0],
            },
            Self::MissingValue | Self::RejectAfterFirst => FanPlan {
                inputs: vec![0],
                outputs: vec![0],
            },
            Self::BusyOutput => FanPlan {
                inputs: vec![usize::from(!available.first().copied().unwrap_or(false))],
                outputs: vec![0],
            },
            Self::WrongCount => FanPlan {
                inputs: values
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| v.as_ref().map(|_| i))
                    .collect(),
                outputs: vec![0],
            },
        })
    }
    fn route(&mut self, values: Vec<String>) -> Result<Vec<String>, &'static str> {
        match self {
            Self::WrongCount => Ok(vec![]),
            Self::EmptyInput | Self::MissingValue | Self::BusyOutput | Self::RejectAfterFirst => {
                Ok(values)
            }
        }
    }
}

#[test]
fn later_plan_failure_preserves_already_committed_output() {
    // Given: one output completes while a second input remains buffered.
    let mut hardware = Fan::new(
        vec![Source(0), Source(1)],
        Invalid::RejectAfterFirst,
        vec![Sink],
    );
    let mut state = <Fan<Source, Invalid, Sink> as Stage<Context>>::start(vec![
        "first".into(),
        "second".into(),
    ]);
    let mut context = Context {
        released: true,
        ..Context::default()
    };
    for _ in 0..7 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    // When: a later routing decision fails after the first receipt was retained.
    let result = hardware.step(&mut context, &mut state);
    // Then: committed effects remain, later work does not run, and the batch is terminal.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Application(
            "later plan failed"
        ))))
    );
    assert_eq!(context.writes, ["first"]);
    assert_eq!(context.reads, [0, 1]);
    assert!(state.is_none());
}

#[test]
fn invalid_plans_cannot_create_unbounded_work_or_use_unfinished_values() {
    // Given: a stalled source and plans that invent work or select its unfinished value.
    for (policy, expected) in [
        (Invalid::EmptyInput, BranchError::EmptyInput),
        (Invalid::MissingValue, BranchError::Unavailable { index: 0 }),
    ] {
        let mut hardware = Fan::new(vec![Source(0)], policy, vec![Sink]);
        let mut state = <Fan<Source, Invalid, Sink> as Stage<Context>>::start(vec!["held".into()]);
        let mut context = Context::default();
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
        // When: the plan crosses the validation boundary.
        let result = hardware.step(&mut context, &mut state);
        // Then: no output interaction occurs and invalid work cannot survive.
        assert_eq!(result, Ok(StepOutcome::Complete(Err(expected))));
        assert!(context.writes.is_empty());
        assert!(state.is_none());
    }
}

#[test]
fn routed_count_mismatch_is_rejected_before_output_execution() {
    // Given: a route returns fewer owned payloads than its validated destination plan.
    let mut hardware = Fan::new(vec![Source(1)], Invalid::WrongCount, vec![Sink]);
    let mut state = <Fan<Source, Invalid, Sink> as Stage<Context>>::start(vec!["owned".into()]);
    let mut context = Context::default();
    assert_eq!(
        hardware.step(&mut context, &mut state),
        Ok(StepOutcome::InProgress)
    );
    // When: the policy produces its payloads.
    let result = hardware.step(&mut context, &mut state);
    // Then: the output stage never starts a malformed invocation.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::OutputCount {
            expected: 1,
            actual: 0
        })))
    );
    assert!(context.writes.is_empty());
}

#[test]
fn occupied_output_cannot_be_overwritten_by_a_later_plan() {
    // Given: one output is incomplete while another input value remains buffered.
    let mut hardware = Fan::new(vec![Source(0), Source(1)], Invalid::BusyOutput, vec![Sink]);
    let mut state = <Fan<Source, Invalid, Sink> as Stage<Context>>::start(vec![
        "first".into(),
        "second".into(),
    ]);
    let mut context = Context {
        released: true,
        ..Context::default()
    };
    for _ in 0..4 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    // When: the next plan tries to reuse the occupied slot.
    let result = hardware.step(&mut context, &mut state);
    // Then: the active child is never overwritten and the invalid batch terminates.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Unavailable {
            index: 0
        })))
    );
    assert!(context.writes.is_empty());
    assert!(state.is_none());
}
