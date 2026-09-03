use pdma_model_contract::semantic::TraceEventKind;

pub(super) fn irq_transitions(trace: &[TraceEventKind]) -> Vec<TraceEventKind> {
    trace
        .iter()
        .copied()
        .filter(|kind| {
            matches!(
                kind,
                TraceEventKind::IrqSourceLatched
                    | TraceEventKind::IrqLineAsserted
                    | TraceEventKind::IrqDeviceAck
                    | TraceEventKind::IrqLineDeasserted
            )
        })
        .collect()
}
