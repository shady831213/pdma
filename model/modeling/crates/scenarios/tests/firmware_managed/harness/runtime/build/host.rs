use super::*;

impl<M: Clone + MemoryAccess> Managed<M>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    /// Assembles one channel and one Host without starting actors.
    pub fn new(memory: M) -> Result<(Self, Host<M>), BuildError> {
        Self::with_channels(memory, 1)
    }

    /// Selects the channel count for the single-Host platform.
    pub fn with_channels(memory: M, channel_count: usize) -> Result<(Self, Host<M>), BuildError> {
        Self::with_addresses(memory, channel_count, platform::ADDRESSES)
    }

    /// Places the single-Host platform at caller-selected register bases.
    pub fn with_addresses(
        memory: M,
        channel_count: usize,
        addresses: ManagementAddresses,
    ) -> Result<(Self, Host<M>), BuildError> {
        let (system, mut hosts) = Self::assemble(
            std::iter::once(memory),
            channel_count,
            pdma_model_firmware::Configuration {
                addresses,
                function_count: 1,
            },
        )?;
        let host = hosts
            .pop()
            .ok_or(BuildError::Platform(PlatformConfigError::FunctionCount))?;
        Ok((system, host))
    }

    pub(crate) fn with_dma_config(
        memory: M,
        config: pdma_model_dma::DmaConfig,
    ) -> Result<(Self, Host<M>), BuildError> {
        let firmware = pdma_model_firmware::Configuration {
            addresses: platform::ADDRESSES,
            function_count: 1,
        };
        let (system, mut hosts) = Self::assemble_spaces(
            memory::FunctionSpaces::new(vec![memory]).map_err(BuildError::Platform)?,
            config.channels,
            (firmware, None, config.capabilities),
        )?;
        let host = hosts
            .pop()
            .ok_or(BuildError::Platform(PlatformConfigError::FunctionCount))?;
        Ok((system, host))
    }
}

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
            runtime: InterruptRuntime::new(Arc::new(SoftwareEnv::new()), source),
        }
    }
}
