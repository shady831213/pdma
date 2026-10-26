use pdma_model_contract::dma::{
    Command, CopyCommandDestinationValues, CopyCommandSourceValues, Event, ScatterGatherEndpoint,
    Segment,
};

use super::super::buffers::Driver;
use super::{Grant, Input, Ports, Space, TestResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings};
use pdma_model_software::dma::ChannelControl;

pub(super) async fn prepare<M: Send>(
    ports: &Ports<'_, M>,
    (grant, [lists, source, destination]): (Grant, [&Space; 3]),
    input: &Input,
) -> TestResult<(
    Driver<M>,
    Command,
    MemoryAddress,
    pdma_model_software::dma::CompletionWord,
)> {
    let Grant {
        channel,
        contexts: [list_id, ring_id, source_id, destination_id],
    } = grant;
    let length = u64::try_from(input.payload.len())?;
    let src = source.allocator.allocate(length, MemoryAlignment::ONE)?;
    let dst = destination
        .allocator
        .allocate(length, MemoryAlignment::ONE)?;
    source
        .allocator
        .memory_access()
        .write(src, MemoryAlignment::ONE, &input.payload)?;
    let source_endpoint = endpoint(lists, (src, input.splits[0], length))?;
    let destination_endpoint = endpoint(lists, (dst, input.splits[1], length))?;
    source.pages.evict((src.raw() + input.fault_offset).into());
    destination
        .pages
        .evict((dst.raw() + input.fault_offset).into());
    let alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let commands = ports
        .allocator
        .allocate(u64::try_from(4 * Command::RECORD_SIZE)?, alignment)?;
    let responses = ports.allocator.allocate(
        u64::try_from(2 * Event::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
    )?;
    ChannelControl::discover_at(&ports.bus, channel, ports.bus.addresses().dma)
        .await?
        .set_ring_context(&ports.bus, ring_id)
        .await?;
    let driver = Channel::setup_at(
        ports.bus.clone(),
        ports.memory.clone(),
        ChannelConfig {
            id: channel,
            rings: Rings {
                command: RingConfig::new(commands.raw(), 4)?,
                event: RingConfig::new(responses.raw(), 2)?,
            },
        },
        ports.bus.addresses().dma,
    )
    .await?;
    let request = Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: input.id,
        source: CopyCommandSourceValues::ScatterGather(source_endpoint),
        source_context: source_id.raw(),
        destination: CopyCommandDestinationValues::ScatterGather(destination_endpoint),
        destination_context: destination_id.raw(),
        copy_length: length,
        sg_context: list_id.raw(),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    let address = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
    ports
        .memory
        .write(address, MemoryAlignment::ONE, &0_u64.to_le_bytes())?;
    let fence = pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
        target: pdma_model_contract::dma::WordTarget {
            address: address.raw(),
            context: ring_id.raw(),
            width: pdma_model_contract::dma::WordTargetWidth::Bits64,
        },
        value: input.id,
    });
    Ok((driver, request, dst, fence))
}

fn endpoint(
    lists: &Space,
    (address, split, length): (MemoryAddress, u64, u64),
) -> TestResult<ScatterGatherEndpoint> {
    let base = lists.allocator.allocate(
        u64::try_from(2 * Segment::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Segment::RECORD_ALIGNMENT)?)?,
    )?;
    for (index, segment) in [
        Segment {
            address: address.raw(),
            length: split,
        },
        Segment {
            address: address.raw() + split,
            length: length - split,
        },
    ]
    .iter()
    .enumerate()
    {
        lists.allocator.memory_access().write(
            (base.raw() + u64::try_from(index * Segment::RECORD_SIZE)?).into(),
            MemoryAlignment::ONE,
            &segment.to_bytes()?,
        )?;
    }
    Ok(ScatterGatherEndpoint {
        list_address: base.raw(),
        count: 2,
        index: 0,
        offset: 0,
    })
}
