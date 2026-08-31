use pdma_model_contract::semantic::ResourceConfig;

#[derive(Debug)]
pub(crate) struct ExecutionUnit {
    resources: ResourceConfig,
    native_steps: usize,
}

impl ExecutionUnit {
    pub(crate) const fn new(resources: ResourceConfig) -> Self {
        Self {
            resources,
            native_steps: 0,
        }
    }

    pub(crate) const fn resources(&self) -> ResourceConfig {
        self.resources
    }

    pub(crate) const fn record_fetch(&mut self) {
        self.native_steps += 1;
    }

    pub(crate) const fn native_steps(&self) -> usize {
        self.native_steps
    }
}
