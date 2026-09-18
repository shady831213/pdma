use super::*;
use pdma_model_contract::{FunctionControlRegisters, FunctionId};
use pdma_model_hw_components::WindowConfigError;

impl<M: Clone> Managed<M> {
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
            memory,
            channel_count,
            pdma_model_firmware::Configuration {
                addresses,
                function_count: 1,
            },
        )?;
        let host = hosts
            .pop()
            .ok_or(BuildError::Window(WindowConfigError::InvalidChannelCount))?;
        Ok((system, host))
    }

    /// Assembles independent Host capabilities over one shared DMA and firmware.
    pub fn with_functions(
        memory: M,
        channel_count: usize,
        function_count: usize,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        Self::assemble(
            memory,
            channel_count,
            pdma_model_firmware::Configuration {
                addresses: platform::ADDRESSES,
                function_count,
            },
        )
    }

    fn assemble(
        memory: M,
        channel_count: usize,
        config: pdma_model_firmware::Configuration,
    ) -> Result<(Self, Vec<Host<M>>), BuildError> {
        let addresses = config.addresses;
        let function_count = config.function_count;
        platform::validate_functions(config, channel_count)?;
        let dma = Dma::new(memory.clone(), channel_count).map_err(BuildError::Dma)?;
        let pipeline = dma.command_loop();
        let window = ChannelWindows::new(channel_count, addresses.dma, addresses.windows)
            .map_err(BuildError::Window)?;
        let invalid = || BuildError::Window(WindowConfigError::AddressUnrepresentable);
        let firmware =
            FirmwareStatus::new(usize::try_from(addresses.firmware).map_err(|_| invalid())?);
        let control = FunctionControls::new(
            usize::try_from(addresses.control).map_err(|_| invalid())?,
            function_count,
        )
        .map_err(BuildError::Window)?;
        let layout = FunctionControlRegisters::new(function_count).map_err(|_| invalid())?;
        let (to_firmware, firmware_source) = pending_interrupt(ControlIrq);
        let mut destinations = Vec::new();
        let mut sources = Vec::new();
        for index in 0..function_count {
            let function = FunctionId::new(u32::try_from(index).map_err(|_| invalid())?);
            let (interrupt, dma_source) = pending_interrupt(HostIrq::Dma(Irq));
            let (to_host, control_source) = pending_interrupt(HostIrq::Control(ControlIrq));
            destinations.push(Destination { interrupt, to_host });
            let offset = u32::try_from(layout.functions().offset(index).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            let host_addresses = ManagementAddresses {
                control: addresses.control.checked_add(offset).ok_or_else(invalid)?,
                ..addresses
            };
            sources.push((
                function,
                host_addresses,
                HostInterrupts {
                    dma: dma_source,
                    control: control_source,
                    next: HostVector::Dma,
                },
            ));
        }
        let shared = Arc::new(Shared {
            addresses,
            hardware: Mutex::new(Hardware {
                dma,
                window,
                firmware,
                control,
            }),
            changed: Notify::new(),
            destinations,
            channel_count,
            to_firmware,
        });
        let hosts = sources
            .into_iter()
            .map(|(function, addresses, source)| Host {
                bus: HostBus {
                    shared: Arc::clone(&shared),
                    function,
                    addresses,
                },
                runtime: InterruptRuntime::new(Arc::new(SoftwareCpu::new()), source),
            })
            .collect();
        let firmware_cpu = Arc::new(SoftwareCpu::new());
        Ok((
            Self {
                shared,
                pipeline,
                firmware_runtime: InterruptRuntime::new(Arc::clone(&firmware_cpu), firmware_source),
                firmware_cpu,
                firmware_memory: memory,
                function_count,
            },
            hosts,
        ))
    }
}
