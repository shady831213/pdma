use super::BridgeError;
use crate::abi::{
    AbiCommand, AbiId, DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadValues, DmaAddress,
};
use crate::semantic::{BufferId, CommandId, CompletionRingId, CopyCommand, CopyRequest, QueueId};
use hardware_interface_runtime::MemoryAddress;

/// Explicit runtime addresses used to encode a semantic copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyRuntimeAddresses {
    source: MemoryAddress,
    destination: MemoryAddress,
}

impl CopyRuntimeAddresses {
    /// Creates explicit source and destination addresses.
    pub const fn new(source: MemoryAddress, destination: MemoryAddress) -> Self {
        Self {
            source,
            destination,
        }
    }

    /// Returns the source address.
    pub const fn source(self) -> MemoryAddress {
        self.source
    }

    /// Returns the destination address.
    pub const fn destination(self) -> MemoryAddress {
        self.destination
    }
}

/// Semantic identities absent from an exact copy descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyBufferContext {
    queue: QueueId,
    completion_ring: CompletionRingId,
    source: BufferId,
    destination: BufferId,
}

impl CopyBufferContext {
    /// Captures the explicit identities needed to rebuild a semantic command.
    pub const fn from_command(command: CopyCommand) -> Self {
        Self {
            queue: command.queue_id(),
            completion_ring: command.completion_ring_id(),
            source: command.request().src(),
            destination: command.request().dst(),
        }
    }
}

/// Converts a semantic copy and explicit runtime addresses to one exact descriptor.
pub fn encode_copy_command(
    command: CopyCommand,
    addresses: CopyRuntimeAddresses,
) -> Result<AbiCommand, BridgeError> {
    let request = command.request();
    let source = add_offset(addresses.source, request.src_start().offset())?;
    let destination = add_offset(addresses.destination, request.dst_start().offset())?;
    let length =
        u32::try_from(request.len()).map_err(|_| BridgeError::SemanticRange { value: u64::MAX })?;
    Ok(AbiCommand::copy(
        AbiId::try_from(command.command_id().raw())?,
        DescriptorSchemaPayloadCopyValues {
            source_dma_address: source.raw(),
            destination_dma_address: destination.raw(),
            length,
        },
    ))
}

/// Rebuilds a semantic copy using caller-supplied identity context.
pub fn decode_copy_command(
    command: AbiCommand,
    context: CopyBufferContext,
) -> Result<(CopyCommand, CopyRuntimeAddresses), BridgeError> {
    let DescriptorSchemaPayloadValues::Copy(copy) = command.payload() else {
        return Err(BridgeError::UnexpectedOpcode);
    };
    let command_id = CommandId::new(command.command_id().try_into_model()?);
    let length = usize::try_from(copy.length).map_err(|_| BridgeError::SemanticRange {
        value: u64::from(copy.length),
    })?;
    let semantic = CopyCommand::new(
        command_id,
        context.queue,
        context.completion_ring,
        CopyRequest::new(context.source, context.destination, length),
    );
    let addresses = CopyRuntimeAddresses::new(
        MemoryAddress::new(copy.source_dma_address),
        MemoryAddress::new(copy.destination_dma_address),
    );
    Ok((semantic, addresses))
}

fn add_offset(base: MemoryAddress, offset: usize) -> Result<DmaAddress, BridgeError> {
    let offset =
        u64::try_from(offset).map_err(|_| BridgeError::SemanticRange { value: u64::MAX })?;
    let raw = base.raw().checked_add(offset).ok_or_else(|| {
        BridgeError::AbiAddress(crate::abi::AbiAddressError::RangeOverflow {
            base: base.raw(),
            length: offset,
        })
    })?;
    Ok(DmaAddress::new(raw))
}
