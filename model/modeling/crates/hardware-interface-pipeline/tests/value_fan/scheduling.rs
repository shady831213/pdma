use crate::{Context, Select, Sink, Source};
use hardware_interface_pipeline::{Fan, FanPlan, FanPolicy, Operation, StepOutcome};
use std::{cell::RefCell, rc::Rc};

struct Observe {
    calls: Rc<RefCell<Vec<(usize, bool)>>>,
    discard_busy: bool,
}
impl FanPolicy<String, String, &'static str> for Observe {
    fn plan(
        &mut self,
        values: &[Option<String>],
        available: &[bool],
    ) -> Result<FanPlan, &'static str> {
        let idle = available.first() == Some(&true);
        self.calls
            .borrow_mut()
            .push((values.iter().flatten().count(), idle));
        if self.discard_busy && !idle {
            return Ok(FanPlan {
                inputs: values
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| v.as_ref().map(|_| i))
                    .collect(),
                outputs: vec![],
            });
        }
        Select.plan(values, available)
    }
    fn route(&mut self, values: Vec<String>) -> Result<Vec<String>, &'static str> {
        if self.discard_busy && self.calls.borrow().last() == Some(&(1, false)) {
            Ok(vec![])
        } else {
            Ok(values)
        }
    }
}

#[test]
fn output_only_work_advances_each_tick_without_empty_plans() {
    // Given: a completed input and a sink requiring two real steps.
    let calls = Rc::new(RefCell::new(vec![]));
    let mut fan = Fan::new(
        vec![Source(1)],
        Observe {
            calls: Rc::clone(&calls),
            discard_busy: false,
        },
        vec![Sink::default()],
    );
    let mut operation = Operation::new(vec!["value".into()]);
    let mut context = Context::default();
    // When: input, routing, and the two output interactions run on separate ticks.
    for _ in 0..3 {
        assert_eq!(
            operation.step(&mut fan, &mut context),
            Ok(StepOutcome::InProgress)
        );
        assert!(context.writes.is_empty());
    }
    let result = operation.step(&mut fan, &mut context);
    // Then: output progress has no empty input/plan detour.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, 5)]))));
    assert_eq!(context.writes, ["value"]);
    assert_eq!(*calls.borrow(), [(1, true)]);
}

#[test]
fn unfinished_inputs_advance_without_empty_policy_callbacks() {
    // Given: the only input cannot yet complete.
    let calls = Rc::new(RefCell::new(vec![]));
    let mut fan = Fan::new(
        vec![Source(0)],
        Observe {
            calls: Rc::clone(&calls),
            discard_busy: false,
        },
        vec![Sink::default()],
    );
    let mut operation = Operation::new(vec!["held".into()]);
    let mut context = Context::default();
    // When: successive bounded steps poll that real input work.
    for _ in 0..4 {
        assert_eq!(
            operation.step(&mut fan, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    // Then: unfinished work progresses without planning absent values.
    assert_eq!(context.reads, [0; 4]);
    assert!(calls.borrow().is_empty());
    assert!(context.writes.is_empty());
}

#[test]
fn buffered_values_are_replanned_while_outputs_are_busy() {
    // Given: two completed values share a two-step sink; a policy may discard the second.
    for discard_busy in [false, true] {
        let calls = Rc::new(RefCell::new(vec![]));
        let mut fan = Fan::new(
            vec![Source(1), Source(2)],
            Observe {
                calls: Rc::clone(&calls),
                discard_busy,
            },
            vec![Sink::default()],
        );
        let mut operation = Operation::new(vec!["first".into(), "second".into()]);
        let mut context = Context::default();
        // When: the finite batch drains, including a plan against a busy output.
        let result = (0..16).find_map(|_| match operation.step(&mut fan, &mut context) {
            Ok(StepOutcome::InProgress) => None,
            other => Some(other),
        });
        // Then: backpressure retains values, and occupied outputs do not suppress disposal.
        let expected = if discard_busy {
            vec![(0, 5)]
        } else {
            vec![(0, 5), (0, 6)]
        };
        assert_eq!(result, Some(Ok(StepOutcome::Complete(Ok(expected)))));
        assert!(calls.borrow().contains(&(1, false)));
        assert!(calls.borrow().iter().all(|(count, _)| *count > 0));
        assert_eq!(context.reads, [1, 2]);
        assert_eq!(
            context.writes,
            if discard_busy {
                vec!["first"]
            } else {
                vec!["first", "second"]
            }
        );
    }
}
