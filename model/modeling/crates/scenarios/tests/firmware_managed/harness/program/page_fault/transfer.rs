use super::super::buffers::Driver;
use super::{Ports, Space, TestResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{ChannelId, Command, ContextId, Endpoint, Response, Segment};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings};
use pdma_model_software::dma::ChannelControl;

pub(super) async fn prepare(
    ports: &Ports<'_>,
    channel: ChannelId,
    [list_id, ring_id, source_id, destination_id]: [ContextId; 4],
    (lists, source, destination): (&Space, &Space, &Space),
) -> TestResult<(Driver, Command, MemoryAddress)> {
    let data: Vec<u8> = (0_u8..=250).cycle().take(600).collect();
    let src = source.allocator.allocate(600, MemoryAlignment::ONE)?;
    let dst = destination.allocator.allocate(600, MemoryAlignment::ONE)?;
    source
        .allocator
        .memory_access()
        .write(src, MemoryAlignment::ONE, &data)?;
    let source_endpoint = endpoint(lists, src, 300, source_id)?;
    let destination_endpoint = endpoint(lists, dst, 200, destination_id)?;
    source.pages.evict((src.raw() + 256).into());
    destination.pages.evict((dst.raw() + 256).into());
    let alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let commands = ports
        .allocator
        .allocate(u64::try_from(2 * Command::RECORD_SIZE)?, alignment)?;
    let responses = ports
        .allocator
        .allocate(u64::try_from(2 * Response::RECORD_SIZE)?, alignment)?;
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
                command: RingConfig::new(commands.raw(), 2)?,
                response: RingConfig::new(responses.raw(), 2)?,
            },
        },
        ports.bus.addresses().dma,
    )
    .await?;
    let request = Command {
        id: 17,
        source: source_endpoint,
        destination: destination_endpoint,
        copy_length: 600,
        sg_context: list_id.raw(),
        ..Command::ZERO
    };
    Ok((driver, request, dst))
}

fn endpoint(
    lists: &Space,
    address: MemoryAddress,
    split: u64,
    context: ContextId,
) -> TestResult<Endpoint> {
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
            length: 600 - split,
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
    Ok(Endpoint {
        context: context.raw(),
        ..Endpoint::scatter_gather(base.raw(), 2)
    })
}
pub(super) fn continuation(
    mut request: pdma_model_contract::dma::Command,
    fault: pdma_model_contract::dma::PageFaultInfo,
) -> super::TestResult<pdma_model_contract::dma::Command> {
    request.copy_length = request
        .copy_length
        .checked_sub(fault.bytes_completed)
        .ok_or("invalid progress")?;
    request.source = request
        .source
        .with_start(fault.source_next())
        .ok_or("source position mode mismatch")?;
    request.destination = request
        .destination
        .with_start(fault.destination_next())
        .ok_or("destination position mode mismatch")?;
    request.id = request.id.checked_add(1).ok_or("command ID overflow")?;
    Ok(request)
}
