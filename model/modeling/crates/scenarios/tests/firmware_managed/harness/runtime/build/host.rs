use super::*;

impl<M> Host<M> {
    pub(super) fn new(
        shared: Arc<Shared<M>>,
        function: FunctionId,
        source: HostInterrupts,
    ) -> Self {
        Self {
            bus: HostBus {
                addresses: shared.addresses,
                shared,
                function,
            },
            runtime: InterruptRuntime::new(Arc::new(SoftwareCpu::new()), source),
        }
    }
}
