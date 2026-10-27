//! Reset-handshake fixtures with suspended accepted DMA work.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CommandRegisters, CopyCommandDestinationValues,
    CopyCommandSourceValues, LinearEndpoint,
};

use hardware_interface_memory::{
    MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory, SharedMemoryAccess,
};
use hardware_interface_pipeline::{AsyncInteractionBackend, HardwareDriver, StepOutcome};
use hardware_interface_register::RegisterAccess;
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_dma::{Dma, Hardware};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct ReadFailureBus {
    model: Model<SharedMemoryAccess>,
    fail: Arc<AtomicBool>,
}

impl RegisterAccess for ReadFailureBus {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(pdma_model_dma::Error::Unmapped);
        }
        self.model.read(offset, response).await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.model.write(offset, request).await
    }
}

pub(super) struct Fixture {
    pub(super) channel: Channel<ReadFailureBus, SharedMemoryAccess>,
    pub(super) model: Model<SharedMemoryAccess>,
    pub(super) fail: Arc<AtomicBool>,
    pub(super) hardware: Hardware,
    pub(super) driver: HardwareDriver<Hardware, Dma<SharedMemoryAccess>>,
}

pub(super) async fn fixture(cpu: &SoftwareCpu) -> Result<Fixture, Box<dyn std::error::Error>> {
    let storage = Arc::new(SharedMemory::new());
    storage.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x2000,
        MemoryAlignment::ONE,
    )?])?;
    let memory = SharedMemoryAccess::from(storage);
    let device = Dma::new(memory.clone(), 2)?;
    let mut hardware = device.hardware();
    let mut driver = HardwareDriver::new();
    let model = Model::new(device);
    let fail = Arc::new(AtomicBool::new(false));
    let mut channel = cpu
        .run_main(Channel::setup(
            ReadFailureBus {
                model: model.clone(),
                fail: Arc::clone(&fail),
            },
            memory,
            ChannelConfig {
                id: ChannelId::new(1),
                rings: Rings {
                    command: RingConfig::new(0x1000, 1)?,
                    event: RingConfig::new(0x2000, 1)?,
                },
            },
        ))
        .await?;
    cpu.run_main(
        channel.submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 71,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0,
                offset: 0,
            }),
            copy_length: 0,
            sg_context: 0,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        })),
    )
    .await?;
    let consumer = u32::try_from(
        ChannelId::new(1)
            .register_offset(2)
            .ok_or("channel offset")?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::CONSUMER.offset(),
    )?;
    let mut accepted = false;
    for _ in 0..128 {
        assert!(matches!(
            model
                .interact(|dma| driver.step(&mut hardware, dma))
                .await?,
            Ok(StepOutcome::InProgress)
        ));
        let mut bytes = [0; 4];
        model.read(consumer, &mut bytes).await?;
        if u32::from_le_bytes(bytes) == 1 {
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    Ok(Fixture {
        channel,
        model,
        fail,
        hardware,
        driver,
    })
}
