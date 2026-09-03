use pdma_model_contract::semantic::TraceEventKind;

use super::{Case, Drive, Driven};
use crate::device::command_pipeline::tests::support::trace_kinds;

pub(super) fn print(case: Case, drive: Drive, driven: &Driven) {
    let observation = &driven.observation;
    let irq_transitions: Vec<_> = trace_kinds(&observation.trace)
        .into_iter()
        .filter(|kind| {
            matches!(
                kind,
                TraceEventKind::IrqSourceLatched
                    | TraceEventKind::IrqLineAsserted
                    | TraceEventKind::IrqLineDeasserted
            )
        })
        .collect();
    println!(
        "HAPPY_MATRIX|route={case:?}|policy={drive:?}|destinations={:?}|completion={:?}|event={:?}|cursors_before_poll={:?}|cursors_after_poll={:?}|irq_before_poll={}|irq_after_poll={}|irq_transitions={irq_transitions:?}|trace={:?}|fetches={}|executions={}|pending={}|terminal={:?}|post_terminal={:?}",
        observation.destinations,
        observation.completion,
        observation.event,
        observation.cursors_before_poll,
        observation.cursors_after_poll,
        observation.irq_before_poll,
        observation.irq_after_poll,
        trace_kinds(&observation.trace),
        driven.fetches,
        driven.executions,
        driven.pending,
        observation.terminal,
        driven.post_terminal,
    );
}
