mod host;
use super::*;
use pdma_model_contract::memory::{FunctionId, MemoryIdentity};

type TimedDma<M> = (
    Dma<RoutedMemory<M>, DmaAccess>,
    Arc<transport::SimulationTimer>,
);

impl<M: Clone + MemoryAccess> Managed<M>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    /// Assembles independent Host capabilities over one shared DMA and firmware.
    pub fn with_functions(
        memory: M,
        channel_count: usize,
        function_count: usize,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        Self::assemble(
            std::iter::repeat_n(memory, function_count),
            channel_count,
            pdma_model_firmware::Configuration {
                addresses: platform::ADDRESSES,
                function_count,
            },
        )
    }

    /// Assigns platform memory capabilities to Function IDs, with PF memory serving control rings.
    pub fn with_function_memory(
        spaces: Vec<M>,
        channel_count: usize,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        let function_count = spaces.len();
        Self::assemble(
            spaces.into_iter(),
            channel_count,
            pdma_model_firmware::Configuration {
                addresses: platform::ADDRESSES,
                function_count,
            },
        )
    }

    fn assemble(
        spaces: impl Iterator<Item = M>,
        channel_count: usize,
        config: pdma_model_firmware::Configuration,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        platform::validate_functions(config, channel_count, None)?;
        Self::assemble_spaces(
            memory::FunctionSpaces::new(spaces.collect()).map_err(BuildError::Platform)?,
            channel_count,
            (
                config,
                None,
                pdma_model_contract::dma::CapabilitiesValues::default(),
            ),
        )
    }

    pub(crate) fn with_context_memory(
        spaces: Vec<(MemoryIdentity, M)>,
        channel_count: usize,
        context: platform::ContextPlatform,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        Self::with_platform_memory((spaces, None), channel_count, context)
    }

    pub(in crate::harness) fn with_platform_memory(
        (spaces, bind): platform::MemoryBinding<M>,
        channel_count: usize,
        context: platform::ContextPlatform,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        let config = pdma_model_firmware::Configuration {
            addresses: ManagementAddresses {
                contexts: Some(0x0013_0000),
                ..platform::ADDRESSES
            },
            function_count: context.function_count,
        };
        Self::assemble_spaces(
            memory::FunctionSpaces {
                backing: spaces,
                bind,
            },
            channel_count,
            (
                config,
                Some(context.capacity),
                pdma_model_contract::dma::CapabilitiesValues::default(),
            ),
        )
    }

    fn configured_dma(
        memory: RoutedMemory<M>,
        channels: usize,
        capabilities: pdma_model_contract::dma::CapabilitiesValues,
    ) -> Result<TimedDma<M>, BuildError> {
        let timer = Arc::new(transport::SimulationTimer::default());
        let dma = Dma::with_config_and_sideband(
            memory,
            pdma_model_dma::DmaConfig {
                channels,
                capabilities,
            },
            |access| access,
        )
        .map_err(BuildError::Dma)?
        .with_timer(timer.clone());
        Ok((dma, timer))
    }

    fn assemble_spaces(
        spaces: memory::FunctionSpaces<M>,
        channel_count: usize,
        (config, capacity, capabilities): (
            pdma_model_firmware::Configuration,
            Option<usize>,
            pdma_model_contract::dma::CapabilitiesValues,
        ),
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        let (addresses, function_count) = (config.addresses, config.function_count);
        platform::validate_functions(config, channel_count, capacity)?;
        let window = ChannelWindows::new(channel_count, addresses.dma, addresses.windows)
            .map_err(BuildError::Window)?;
        let windows = Arc::new(std::sync::Mutex::new(window));
        let contexts = platform::contexts(capacity, addresses.contexts, channel_count)?;
        let routed = match &contexts {
            Some(contexts) => RoutedMemory::with_contexts(
                Arc::clone(&windows),
                Arc::clone(contexts),
                spaces.clone(),
            ),
            None => RoutedMemory::new(
                Arc::clone(&windows),
                spaces
                    .backing
                    .iter()
                    .map(|(_, memory)| memory.clone())
                    .collect(),
            )
            .map_err(BuildError::Platform)?,
        };
        let (dma, timer) = Self::configured_dma(routed, channel_count, capabilities)?;
        let hardware = Box::new(dma.hardware());
        let invalid = || BuildError::Platform(PlatformConfigError::AddressSpace);
        let firmware =
            FirmwareStatus::new(usize::try_from(addresses.firmware).map_err(|_| invalid())?);
        let control = Arc::new(std::sync::Mutex::new(ControlChannel::new(
            usize::try_from(addresses.control).map_err(|_| invalid())?,
            platform::PF,
        )));
        let firmware_memory = memory::ControlMemory {
            control: Arc::clone(&control),
            spaces,
        };
        let (to_firmware, firmware_source) = pending_interrupt(ControlIrq);
        let (to_host, control_source) = pending_interrupt(HostIrq::Control(ControlIrq));
        let mut control_source = Some(control_source);
        let (to_events, event_source) = pending_interrupt(HostIrq::Event(ControlIrq));
        let mut event_source = Some(event_source);
        let mut destinations = Vec::new();
        let mut sources = Vec::new();
        for index in 0..function_count {
            let function = FunctionId::new(
                u32::try_from(index)
                    .map_err(|_| BuildError::Platform(PlatformConfigError::FunctionCount))?,
            );
            let (interrupt, dma_source) = pending_interrupt(HostIrq::Dma(Irq));
            destinations.push(interrupt);
            sources.push((
                function,
                HostInterrupts {
                    dma: dma_source,
                    control: if function == platform::PF {
                        control_source.take()
                    } else {
                        None
                    },
                    event: if function == platform::PF {
                        event_source.take()
                    } else {
                        None
                    },
                    next: HostVector::Dma,
                },
            ));
        }
        let shared = Arc::new(Shared {
            addresses,
            hardware: Mutex::new(Hardware {
                dma,
                windows,
                firmware,
                control,
                contexts,
            }),
            destinations,
            channel_count,
            to_firmware,
            to_host,
            to_events,
            timer,
        });
        let hosts = sources
            .into_iter()
            .map(|(function, source)| Host::new(Arc::clone(&shared), function, source))
            .collect();
        let firmware_cpu = Arc::new(SoftwareCpu::new());
        Ok((
            Self {
                shared,
                hardware,
                firmware_runtime: InterruptRuntime::new(Arc::clone(&firmware_cpu), firmware_source),
                firmware_cpu,
                firmware_memory,
                function_count,
            },
            hosts,
        ))
    }
}
