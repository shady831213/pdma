use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context, Job, finish},
};
use hardware_interface_pipeline::{FanIn, Stage, StepOutcome};
type Gather = FanIn<Candidate, Policy, Accept>;
#[test]
fn selection_consumes_only_the_granted_candidate() {
    // Given: three non-consuming candidate stages and queued jobs on two channels.
    let mut hardware = FanIn::new(
        (0..3).map(Candidate).collect(),
        Policy::Fixed,
        Accept::Normal,
    );
    let mut state = Gather::start(vec![(); 3]);
    let mut context = Context::with_jobs(&[1, 2]);
    // When: completed values are planned and one output accepts work.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: all candidates were observed, only the selected queue was consumed.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![(
            0,
            Some(Job {
                channel: 1,
                value: 7
            })
        )])))
    );
    assert_eq!(context.observed, [0, 1, 2]);
    assert_eq!(context.accepted, [1]);
    assert_eq!(
        context.queues.get(2).map(std::collections::VecDeque::len),
        Some(1)
    );
}
#[test]
fn round_robin_wraps_across_repeated_operations() {
    // Given: reusable policy state and three queued channels.
    let mut hardware = FanIn::new(
        (0..3).map(Candidate).collect(),
        Policy::RoundRobin(2),
        Accept::Normal,
    );
    let mut context = Context::with_jobs(&[0, 1, 2, 0]);
    // When: four independent batches use the same hardware.
    for _ in 0..4 {
        let mut state = Gather::start(vec![(); 3]);
        assert!(matches!(
            finish(&mut hardware, &mut state, &mut context),
            Ok(StepOutcome::Complete(Ok(_)))
        ));
    }
    // Then: policy position survives invocation disposal and wraps.
    assert_eq!(context.accepted, [2, 0, 1, 0]);
}
#[test]
fn fixed_priority_restarts_at_the_first_candidate() {
    // Given: two jobs at the first channel and a peer waiting.
    let mut hardware = FanIn::new(
        (0..2).map(Candidate).collect(),
        Policy::Fixed,
        Accept::Normal,
    );
    let mut context = Context::with_jobs(&[0, 0, 1]);
    // When: two batches are executed.
    for _ in 0..2 {
        let mut state = Gather::start(vec![(); 2]);
        assert!(matches!(
            finish(&mut hardware, &mut state, &mut context),
            Ok(StepOutcome::Complete(Ok(_)))
        ));
    }
    // Then: fixed priority chooses the first eligible candidate each time.
    assert_eq!(context.accepted, [0, 0]);
}
#[test]
fn idle_batch_completes_without_acceptance_and_later_batch_rechecks() {
    // Given: a candidate observes an empty queue.
    let mut hardware = FanIn::new(vec![Candidate(0)], Policy::Fixed, Accept::Normal);
    let mut context = Context::default();
    let mut state = Gather::start(vec![()]);
    // When: the idle batch completes and a new batch follows software submission.
    let idle = finish(&mut hardware, &mut state, &mut context);
    context.queues.push(
        [Job {
            channel: 0,
            value: 9,
        }]
        .into(),
    );
    let mut next = Gather::start(vec![()]);
    let result = finish(&mut hardware, &mut next, &mut context);
    // Then: no work was invented during idle and new candidate values see the submission.
    assert_eq!(idle, Ok(StepOutcome::Complete(Ok(vec![]))));
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![(
            0,
            Some(Job {
                channel: 0,
                value: 9
            })
        )])))
    );
    assert_eq!(context.accepted, [0]);
}
