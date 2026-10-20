use super::super::buffers::Driver;
use super::{Grant, Input, Ports, Space, TestResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{Command, ContextId, CopyCommand, Endpoint, Response, Segment};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings};
use pdma_model_software::dma::ChannelControl;

pub(super) async fn prepare<M: Send>(
    ports: &Ports<'_, M>,
    (grant, [lists, source, destination]): (Grant, [&Space; 3]),
    input: &Input,
) -> TestResult<(Driver<M>, Command, MemoryAddress)> {
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
    let source_endpoint = endpoint(lists, (src, input.splits[0], length), source_id)?;
    let destination_endpoint = endpoint(lists, (dst, input.splits[1], length), destination_id)?;
    source.pages.evict((src.raw() + input.fault_offset).into());
    destination
        .pages
        .evict((dst.raw() + input.fault_offset).into());
    let alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let commands = ports
        .allocator
        .allocate(u64::try_from(2 * Command::RECORD_SIZE)?, alignment)?;
    let responses = ports.allocator.allocate(
        u64::try_from(2 * Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
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
                command: RingConfig::new(commands.raw(), 2)?,
                response: RingConfig::new(responses.raw(), 2)?,
            },
        },
        ports.bus.addresses().dma,
    )
    .await?;
    let request = Command::copy(
        input.id,
        CopyCommand {
            source: source_endpoint,
            destination: destination_endpoint,
            copy_length: length,
            sg_context: list_id.raw(),
        },
    );
    Ok((driver, request, dst))
}

fn endpoint(
    lists: &Space,
    (address, split, length): (MemoryAddress, u64, u64),
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
    Ok(Endpoint {
        context: context.raw(),
        ..Endpoint::scatter_gather(base.raw(), 2)
    })
}
