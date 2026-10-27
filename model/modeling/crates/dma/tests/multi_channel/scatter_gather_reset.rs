use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CopyCommandDestinationValues, CopyCommandSourceValues,
    DmaControl, EventRegisters, LinearEndpoint, ScatterGatherEndpoint, Segment,
};

mod observed;

use super::support::{TestResult, completion, configure, publish_fence, word};
use hardware_interface_io::memory::MemoryRegion;
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_pipeline::StepOutcome;
use observed::Observed;
use pdma_model_dma::Dma;

#[derive(Clone, Copy, Debug)]
enum Boundary {
    SourceList,
    DestinationList,
    PayloadRead,
    PrefixWritten,
    PayloadWritten,
}

impl Boundary {
    const fn reached(self, memory: &Observed) -> bool {
        match self {
            Self::SourceList => memory.source_list_reads.get() == 1,
            Self::DestinationList => memory.destination_list_reads.get() == 1,
            Self::PayloadRead => memory.payload_reads.get() == 1,
            Self::PrefixWritten => memory.payload_writes.get() == 1,
            Self::PayloadWritten => memory.payload_writes.get() == 2,
        }
    }
}

fn setup() -> TestResult<Dma<Observed>> {
    let memory = Observed::default();
    memory.memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory, 1)?;
    configure(&mut device, 0)?;
    device
        .memory
        .memory
        .write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, &[7; 512])?;
    for (list, payload) in [(0x1800, 0x1200), (0x1900, 0x1400)] {
        for index in 0..2_u64 {
            device.memory.memory.write(
                MemoryAddress::new(list + index * u64::try_from(Segment::RECORD_SIZE)?),
                MemoryAlignment::ONE,
                &Segment {
                    address: payload + index * 256,
                    length: 256,
                }
                .to_bytes()?,
            )?;
        }
    }
    publish(
        &mut device,
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 7,
            copy_length: 512,
            sg_context: 0,
            source: CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1800,
                count: 2,
                index: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1900,
                count: 2,
                index: 0,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    Ok(device)
}

fn publish(device: &mut Dma<Observed>, command: Command) -> TestResult {
    device.memory.memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    publish_fence(device, 0, command.id())
}

fn snapshot(device: &Dma<Observed>, address: u64) -> TestResult<[u8; 512]> {
    let mut bytes = [0; 512];
    device.memory.memory.read(
        MemoryAddress::new(address),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    Ok(bytes)
}

#[test]
fn reset_stops_sg_interactions_before_completion_and_keeps_them_stopped() -> TestResult {
    for boundary in [
        Boundary::SourceList,
        Boundary::DestinationList,
        Boundary::PayloadRead,
        Boundary::PrefixWritten,
        Boundary::PayloadWritten,
    ] {
        for reset in [
            ChannelId::new(0).admin_offset(1).ok_or("offset")?,
            (pdma_model_contract::dma::DmaDiscovery::EXTENT
                + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
                + DmaControl::RESET.offset(),
        ] {
            // Given: an SG command paused at an observed list, payload or publication boundary.
            let mut device = setup()?;
            let mut hardware = device.hardware();
            let mut operation = hardware_interface_pipeline::HardwareDriver::new();
            for _ in 0..256 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
                if boundary.reached(&device.memory) {
                    break;
                }
            }
            assert!(
                boundary.reached(&device.memory),
                "missing boundary {boundary:?}"
            );
            let before = snapshot(&device, 0x1400)?;
            let accesses = device.memory.interactions.get();
            // When: reset completes through the same retained hardware operation.
            device.write(reset, &1_u32.to_le_bytes())?;
            assert_eq!(word(&mut device, reset)?, 1);
            for _ in 0..32 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
                if word(&mut device, reset)? == 0 {
                    break;
                }
            }
            assert_eq!(word(&mut device, reset)?, 0);
            for _ in 0..64 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
            }
            // Then: no old memory access resumes, and fresh work still completes once.
            assert_eq!(device.memory.interactions.get(), accesses);
            assert_eq!(snapshot(&device, 0x1400)?, before);
            let producer = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset();
            assert_eq!(word(&mut device, producer)?, 0);
            assert!(!device.irq());
            configure(&mut device, 0)?;
            publish(
                &mut device,
                Command::Copy(pdma_model_contract::dma::CopyCommand {
                    id: 99,
                    copy_length: 512,
                    sg_context: 0,
                    source: CopyCommandSourceValues::Linear(LinearEndpoint {
                        address: 0x1200,
                        offset: 0,
                    }),
                    destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                        address: 0x1600,
                        offset: 0,
                    }),
                    ..pdma_model_contract::dma::CopyCommand::ZERO
                }),
            )?;
            for _ in 0..256 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
            }
            assert_eq!(word(&mut device, producer)?, 0);
            assert_eq!(snapshot(&device, 0x1400)?, before);
            assert_eq!(snapshot(&device, 0x1600)?, [7; 512]);
            assert_eq!(completion(&device, 0)?, 100);
        }
    }
    Ok(())
}
