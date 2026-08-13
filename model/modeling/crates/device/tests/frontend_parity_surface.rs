//! Real semantic/exact frontend parity and adverse retirement surface.

#[path = "frontend_parity_surface/adverse.rs"]
/// Adverse retirement observation.
pub mod adverse;
#[path = "bus_submission/common.rs"]
/// Shared exact-frontend fixtures.
pub mod common;
#[path = "frontend_parity_surface/exact.rs"]
/// Exact frontend driver.
pub mod exact;
#[path = "bus_submission/operations.rs"]
/// Exact operation cases.
pub mod operations;
#[path = "frontend_parity_surface/semantic.rs"]
/// Semantic frontend driver.
pub mod semantic;

#[path = "bus_retirement/cases.rs"]
/// Existing retirement cases compiled against the shared fixture.
pub mod cases;

#[path = "bus_retirement/support.rs"]
/// Retirement fixture support.
pub mod support;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[tokio::test(flavor = "current_thread")]
async fn task17_frontend_parity_surface() -> TestResult {
    let mut semantic_results = Vec::new();
    let mut exact_results = Vec::new();
    let mut semantic_traces = Vec::new();
    let mut exact_traces = Vec::new();
    let mut semantic_retirements = 0;
    let mut exact_retirements = 0;
    let mut native_step_observations = Vec::new();
    for operation in operations::OperationCase::ALL {
        let semantic = semantic::observe(operation)?;
        let exact = exact::observe(operation).await?;
        semantic_retirements += semantic.retirements;
        exact_retirements += exact.retirements;
        native_step_observations.extend([semantic.executions, exact.executions]);
        semantic_results.push(semantic.destinations);
        exact_results.push(exact.destinations);
        semantic_traces.push(semantic.traces);
        exact_traces.push(exact.traces);
    }
    let exact_arbitration = operations::multi_queue_arbitration().await?;
    let semantic_arbitration = semantic::arbitration()?;
    let bytes_equal = semantic_results == exact_results;
    let traces_equal = semantic_traces == exact_traces;
    let arbitration_equal = exact_arbitration == semantic_arbitration;
    let native_steps = native_step_observations
        .first()
        .copied()
        .ok_or("operation inventory must produce a native-step observation")?;
    assert!(
        native_step_observations
            .iter()
            .all(|steps| *steps == native_steps)
    );
    assert_eq!(semantic_retirements, exact_retirements);
    let retirements = exact_retirements;
    assert_eq!(semantic_results, exact_results);
    assert_eq!(semantic_traces, exact_traces);
    assert_eq!(exact_arbitration, semantic_arbitration);
    assert!(bytes_equal && traces_equal && arbitration_equal);
    assert_eq!(native_steps, 1);
    println!(
        "operations={};semantic_results={};exact_results={};bytes_equal={bytes_equal};traces_equal={traces_equal};arbitration_equal={arbitration_equal};frontends=semantic,exact;native_steps={native_steps};retirements={retirements}",
        operations::OperationCase::ALL.len(),
        semantic_retirements,
        exact_retirements,
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn task17_frontend_parity_adverse() -> TestResult {
    let (
        pending_retirements,
        executions,
        retries,
        published_after_space,
        trace_before_irq,
        fault_reexecuted,
        consume_requeue,
        unchanged_before_publish,
    ) = adverse::observe().await?;
    assert_eq!(pending_retirements, 1);
    assert_eq!(executions, 1);
    assert_eq!(retries, 1);
    assert!(published_after_space);
    assert!(trace_before_irq);
    assert!(!fault_reexecuted);
    assert!(consume_requeue);
    assert!(unchanged_before_publish);
    println!(
        "pending_retirements={pending_retirements};executions={executions};retries={retries};published_after_space={published_after_space};trace_before_irq={trace_before_irq};fault_reexecuted={fault_reexecuted};consume_requeue={consume_requeue};unchanged_before_publish={unchanged_before_publish}"
    );
    Ok(())
}
