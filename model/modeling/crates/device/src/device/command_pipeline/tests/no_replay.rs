mod completion;
mod execution;
mod fault;

pub(super) fn no_replay() {
    execution::pressure_revisits_only_execution();
    completion::pressure_revisits_only_retirement();
    fault::pressure_revisits_only_fault_retirement();
    println!(
        "no_replay execution_pending=3 completion_pending=2 fault_pending=3 pressured_fetches=1/1/1 pressured_executions=1/1/1 total_fetches=1/2/1 total_executions=1/2/1 records=1/2/1 cursors=literal irq_transitions=literal trace_sequences=literal post_terminal=Exhausted duplicated_side_effects=0"
    );
}
