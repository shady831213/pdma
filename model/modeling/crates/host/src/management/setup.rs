use super::{Management, ManagementConfig};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, RegisterAccess};
use hardware_interface_schema::Encode;
use pdma_model_contract::{ControlQueue, ControlRegisters, ControlRingState, Ring};
use pdma_model_software::{control::ControlQueueLayout, ring::SoftRing};

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Configures empty control rings after firmware Ready, before any DMA grant.
    ///
    /// Geometry becomes immutable on enable. Interrupted setup may leave committed
    /// configuration: retain ring storage until the subsystem is stopped/reset.
    pub async fn setup(
        bus: B,
        memory: M,
        config: ManagementConfig,
    ) -> Result<Self, Error<B::Error>> {
        let ManagementConfig {
            rings,
            interrupt,
            addresses,
        } = config;
        let client = Self {
            addresses,
            bus,
            memory,
            layout: ControlQueueLayout::new(rings.state).map_err(Error::Ring)?,
            interrupt,
            command: SoftRing::new(rings.command),
            response: SoftRing::new(rings.response),
            submission: None,
            completion: None,
            outstanding: Vec::new(),
            capacity: usize::try_from(rings.command.capacity()).map_err(|_| Error::Geometry)?,
            next_id: 1,
        };
        if client.read(ControlRegisters::ENABLE.offset()).await? != 0 {
            return Err(Error::ManagementBusy);
        }
        let state = ControlQueue {
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
            .write_typed(
                rings.state,
                MemoryAlignment::try_new(
                    u64::try_from(ControlQueue::RECORD_ALIGNMENT).map_err(|_| Error::Geometry)?,
                )
                .map_err(|_| Error::Geometry)?,
                &state.encode().map_err(Error::Codec)?,
            )
            .map_err(Error::Memory)?;
        client
            .write(
                ControlRegisters::BOOTSTRAP.offset(),
                &rings.state.raw().to_le_bytes(),
            )
            .await?;
        client
            .write(ControlRegisters::ENABLE.offset(), &1_u32.to_le_bytes())
            .await?;
        client.kick().await?;
        Ok(client)
    }
}
