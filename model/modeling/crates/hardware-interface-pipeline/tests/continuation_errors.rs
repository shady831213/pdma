//! Accepted child continuations survive graph and driving failures.
use hardware_interface_pipeline::{
    BranchError, Fan, FanPlan, FanPolicy, Operation, RunError, Stage, StepOutcome, StepResult, Then,
};
mod continuation_errors {
    mod settling;
}

#[derive(Default)]
struct Child {
    pending: Option<u8>,
    raw: bool,
    immediate: bool,
    raw_on_finish: bool,
    finish_error: Option<u8>,
}
impl Stage<Vec<u8>> for Child {
    type Input = u8;
    type Output = u8;
    type Error = u8;
    fn step(&mut self, effects: &mut Vec<u8>, input: &mut Option<u8>) -> StepResult<u8, u8> {
        if self.raw {
            self.raw = false;
            self.pending = input.take();
            return Err(RunError::Exhausted);
        }
        if self.pending.is_some() && std::mem::take(&mut self.raw_on_finish) {
            return Err(RunError::BudgetExhausted);
        }
        if let Some(value) = self.pending.take() {
            effects.push(value);
            return Ok(StepOutcome::Complete(
                self.finish_error.map_or(Ok(value), Err),
            ));
        }
        if let Some(value) = input.take() {
            if value == 0 {
                return Ok(StepOutcome::Complete(Err(value)));
            }
            if self.immediate {
                effects.push(value);
                return Ok(StepOutcome::Complete(Ok(value)));
            }
            self.pending = Some(value);
        }
        Ok(StepOutcome::InProgress)
    }
}

struct RejectPlan;
impl FanPolicy<u8, u8, u8> for RejectPlan {
    fn plan(&mut self, _: &[Option<u8>], _: &[bool]) -> Result<FanPlan, u8> {
        Err(42)
    }
    fn route(&mut self, _: Vec<u8>) -> Result<Vec<u8>, u8> {
        Err(43)
    }
}

#[test]
fn policy_error_waits_for_accepted_input() {
    // Given: one completed source and one accepted unfinished source.
    let mut graph = Fan::new(
        vec![
            Child {
                immediate: true,
                ..Child::default()
            },
            Child::default(),
        ],
        RejectPlan,
        Vec::<Child>::new(),
    );
    let mut effects = Vec::new();
    let mut input = Some(vec![1, 2]);
    // When: planning fails while the second source owns work.
    assert_eq!(
        graph.step(&mut effects, &mut input),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(
        graph.step(&mut effects, &mut input),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(effects, [1]);
    let result = graph.step(&mut effects, &mut input);
    // Then: the first policy error arrives after the outstanding source completes.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Application(42))))
    );
    assert_eq!(effects, [1, 2]);
}

struct Broadcast;
impl FanPolicy<u8, u8, u8> for Broadcast {
    fn plan(&mut self, _: &[Option<u8>], _: &[bool]) -> Result<FanPlan, u8> {
        Ok(FanPlan {
            inputs: vec![0],
            outputs: vec![0, 1, 2],
        })
    }
    fn route(&mut self, _: Vec<u8>) -> Result<Vec<u8>, u8> {
        Ok(vec![7, 0, 9])
    }
}

#[test]
fn output_error_settles_accepted_peer_and_withdraws_unaccepted_offer() {
    // Given: routing offers three output requests, the second of which fails.
    let mut graph = Fan::new(
        vec![Child {
            immediate: true,
            ..Child::default()
        }],
        Broadcast,
        vec![Child::default(), Child::default(), Child::default()],
    );
    let mut effects = Vec::new();
    let mut input = Some(vec![1]);
    // When: the graph is driven through output failure to its terminal result.
    let result = (0..8).find_map(|_| match graph.step(&mut effects, &mut input).unwrap() {
        StepOutcome::InProgress => None,
        StepOutcome::Complete(result) => Some(result),
    });
    // Then: accepted output commits and the never-accepted third output does not.
    assert_eq!(result, Some(Err(BranchError::Application(0))));
    assert_eq!(effects, [1, 7]);
}

#[test]
fn raw_fan_error_retains_accepted_child_and_unoffered_peer() {
    // Given: the first input accepts before reporting a driving error.
    let mut graph = Fan::new(
        vec![
            Child {
                raw: true,
                ..Child::default()
            },
            Child::default(),
        ],
        Discard,
        Vec::<Child>::new(),
    );
    let mut effects = Vec::new();
    let mut input = Some(vec![3, 4]);
    // When: driving resumes after the raw error.
    assert_eq!(
        graph.step(&mut effects, &mut input),
        Err(RunError::Exhausted)
    );
    let result = (0..8).find_map(|_| match graph.step(&mut effects, &mut input).unwrap() {
        StepOutcome::InProgress => None,
        StepOutcome::Complete(result) => Some(result),
    });
    // Then: both original requests finish once.
    assert_eq!(result, Some(Ok(vec![])));
    assert_eq!(effects, [3, 4]);
}
struct Discard;
impl FanPolicy<u8, u8, u8> for Discard {
    fn plan(&mut self, values: &[Option<u8>], _: &[bool]) -> Result<FanPlan, u8> {
        Ok(FanPlan {
            inputs: values
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|_| i))
                .collect(),
            outputs: vec![],
        })
    }
    fn route(&mut self, _: Vec<u8>) -> Result<Vec<u8>, u8> {
        Ok(vec![])
    }
}

#[test]
fn fan_error_settles_accepted_peer_before_completion_and_reuse() {
    // Given: first child accepts while the second immediately fails.
    let mut graph = Fan::new(
        vec![Child::default(), Child::default()],
        Discard,
        Vec::<Child>::new(),
    );
    let mut effects = Vec::new();
    let mut input = Some(vec![7, 0]);
    // When: the graph reports a domain error.
    assert_eq!(
        graph.step(&mut effects, &mut input),
        Ok(StepOutcome::InProgress)
    );
    let result = graph.step(&mut effects, &mut input);
    // Then: accepted work completes before the error and hardware can be reused.
    assert_eq!(effects, [7]);
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Application(0))))
    );
    input = Some(vec![8, 9]);
    let result = (0..8).find_map(|_| match graph.step(&mut effects, &mut input).unwrap() {
        StepOutcome::InProgress => None,
        StepOutcome::Complete(result) => Some(result),
    });
    assert_eq!(result, Some(Ok(vec![])));
    assert_eq!(effects, [7, 8, 9]);
}

#[test]
fn raw_error_preserves_then_and_operation_continuation() {
    // Given: a child accepts its offer before a driving failure.
    let mut graph = Then::new(
        Child {
            raw: true,
            ..Child::default()
        },
        Child::default(),
    );
    let mut operation = Operation::new(5);
    let mut effects = Vec::new();
    // When: the failure is delivered and the same operation resumes.
    assert_eq!(
        operation.step(&mut graph, &mut effects),
        Err(RunError::Exhausted)
    );
    assert!(!operation.is_complete());
    let result = (0..8).find_map(
        |_| match operation.step(&mut graph, &mut effects).unwrap() {
            StepOutcome::InProgress => None,
            StepOutcome::Complete(result) => Some(result),
        },
    );
    // Then: both accepted stages finish without replaying admission.
    assert_eq!(result, Some(Ok(5)));
    assert_eq!(effects, [5, 5]);
}
