use crate::device::DeviceCore;
use pdma_model_contract::semantic::TraceEvent;

impl DeviceCore {
    pub(crate) const fn enable_tracing(&mut self) {
        self.trace.enable();
    }

    pub(crate) const fn disable_tracing(&mut self) {
        self.trace.disable();
    }

    pub(crate) fn clear_trace(&mut self) {
        self.trace.clear();
    }

    pub(crate) fn trace_events(&self) -> Vec<TraceEvent> {
        self.trace.events().to_vec()
    }

    pub(crate) fn trace_json(&self) -> String {
        self.trace.to_json()
    }

    pub(crate) const fn native_steps(&self) -> usize {
        self.execution.native_steps()
    }
}
