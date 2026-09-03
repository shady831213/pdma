use pdma_model_contract::semantic::{
    EpochIrqSource, IrqCause, IrqLine, IrqSnapshot, IrqVector, TraceEventKind,
};

pub(super) const fn expected_irq(source: EpochIrqSource) -> IrqSnapshot {
    IrqSnapshot::new(IrqCause::new(IrqLine::new(IrqVector::new(0)), source), true)
}

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
