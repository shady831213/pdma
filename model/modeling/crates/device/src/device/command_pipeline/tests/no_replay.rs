mod completion;
mod execution;
mod fault;
mod report;

pub(super) fn no_replay() {
    execution::pressure_revisits_only_execution();
    completion::pressure_revisits_only_retirement();
    fault::pressure_revisits_only_fault_retirement();
}
