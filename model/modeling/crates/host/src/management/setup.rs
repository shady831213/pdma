use super::{Management, ManagementConfig};
use crate::Error;
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_schema::Encode;
use pdma_model_contract::control::{ControlQueue, ControlRingState};
use pdma_model_contract::hw_components::{
    context::ContextAttributeRegisters,
    control::{ControlConfiguration, ControlRegisters},
};
use pdma_model_contract::ring::Ring;
use pdma_model_software::{control::ControlQueueLayout, ring::SoftRing};

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Configures empty control rings after firmware Ready, before any DMA grant.
    ///
    /// Command/response geometry freezes on enable; failed event storage has explicit recovery.
    /// Interrupted setup may leave committed
    /// configuration: retain ring storage until the subsystem is stopped/reset.
    pub async fn setup(
        bus: B,
        memory: M,
        config: ManagementConfig,
    ) -> Result<Self, Error<B::Error, M::Error>> {
        Self::setup_with_context(
            bus,
            memory,
            (
                config,
                pdma_model_contract::memory::ContextAttributes::DEFAULT,
            ),
        )
        .await
    }

    /// Selects independent control-memory attributes before enabling its immutable configuration.
    pub async fn setup_with_context(
        bus: B,
        memory: M,
        (config, attributes): (
            ManagementConfig,
            pdma_model_contract::memory::ContextAttributes,
        ),
    ) -> Result<Self, Error<B::Error, M::Error>> {
        attributes.validate().map_err(|_| {
            Error::ManagementStatus(
                pdma_model_contract::control::ControlResponseStatus::InvalidAttributes,
            )
        })?;
        let ManagementConfig {
            rings,
            interrupt,
            event_interrupt,
            addresses,
        } = config;
        let client = Self {
            addresses,
            bus,
            memory,
            layout: ControlQueueLayout::new(rings.state).map_err(Error::Ring)?,
            interrupt,
            event_interrupt,
            event_state: hardware_interface_io::memory::MemoryAddress::new(
                rings
                    .state
                    .raw()
                    .checked_add(
                        u64::try_from(ControlQueue::EVENT_OFFSET).map_err(|_| Error::Geometry)?,
                    )
                    .ok_or(Error::Geometry)?,
            ),
            event: SoftRing::new(rings.event),
            command: SoftRing::new(rings.command),
            response: SoftRing::new(rings.response),
            submission: None,
            completion: None,
            outstanding: Vec::new(),
            capacity: usize::try_from(rings.command.capacity()).map_err(|_| Error::Geometry)?,
            next_id: 1,
        };
        let configuration = ControlRegisters::CONFIG.offset();
        let enable = configuration + ControlConfiguration::ENABLE.offset();
        if client.read(enable).await? != 0 {
            return Err(Error::ManagementBusy);
        }
        let state = ControlQueue {
            event: ControlRingState {
                base: rings.event.base(),
                capacity: rings.event.capacity(),
                producer: 0,
                consumer: 0,
            },
            command: ControlRingState {
                base: rings.command.base(),
                capacity: rings.command.capacity(),
                producer: 0,
                consumer: 0,
            },
            response: ControlRingState {
                base: rings.response.base(),
                capacity: rings.response.capacity(),
                producer: 0,
                consumer: 0,
            },
        };
        client
            .memory
            .write_buffer(
                rings.state,
                MemoryAlignment::try_new(
                    u64::try_from(ControlQueue::RECORD_ALIGNMENT).map_err(|_| Error::Geometry)?,
                )
                .map_err(|_| Error::Geometry)?,
                &state.encode().map_err(Error::Codec)?,
            )
            .map_err(Error::Memory)?;
        let attribute_base = configuration + ControlConfiguration::ATTRIBUTES.offset();
        client
            .write(
                attribute_base + ContextAttributeRegisters::ADDRESS_SPACE.offset(),
                &attributes.address_space.raw().to_le_bytes(),
            )
            .await?;
        client
            .write(
                attribute_base + ContextAttributeRegisters::PASID.offset(),
                &attributes.pasid.to_le_bytes(),
            )
            .await?;
        client
            .write(
                configuration + ControlConfiguration::BOOTSTRAP.offset(),
                &rings.state.raw().to_le_bytes(),
            )
            .await?;
        client.write(enable, &1_u32.to_le_bytes()).await?;
        client.kick().await?;
        Ok(client)
    }
}
