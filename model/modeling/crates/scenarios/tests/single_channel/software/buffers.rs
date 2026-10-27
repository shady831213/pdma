use hardware_interface_io::memory::MemoryAccessError;
use hardware_interface_io::memory::SharedMemoryAllocator;
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment, SharedMemoryAccess};
use pdma_model_contract::dma::Segment;
use pdma_model_contract::dma::{CopyCommandSourceValues, LinearEndpoint, ScatterGatherEndpoint};
use std::{error::Error, io};

use super::BufferLayout;

pub(super) struct Buffer {
    pub(super) endpoint: CopyCommandSourceValues,
    pub(super) segments: Vec<Segment>,
}

pub(super) struct Builder<'a> {
    pub(super) allocator: &'a SharedMemoryAllocator,
    pub(super) memory: &'a SharedMemoryAccess<(), MemoryAccessError>,
}

impl Builder<'_> {
    pub(super) fn build(
        &self,
        layout: BufferLayout<'_>,
        data: &[u8],
    ) -> Result<Buffer, Box<dyn Error + Send + Sync>> {
        let linear_lengths = [data.len()];
        let lengths = match layout {
            BufferLayout::Linear => &linear_lengths,
            BufferLayout::ScatterGather(lengths) => lengths,
        };
        let mut remaining = data;
        let mut segments = Vec::with_capacity(lengths.len());
        for &length in lengths {
            let (bytes, rest) = remaining
                .split_at_checked(length)
                .ok_or_else(|| io::Error::other("buffer layout exceeds payload length"))?;
            remaining = rest;
            let address = self
                .allocator
                .allocate(u64::try_from(length)?, MemoryAlignment::ONE)?;
            if length > 0 {
                self.memory.write(address, MemoryAlignment::ONE, bytes)?;
            }
            segments.push(Segment {
                address: address.raw(),
                length: u64::try_from(length)?,
            });
        }
        if !remaining.is_empty() {
            return Err(io::Error::other("buffer layout does not cover payload").into());
        }
        let endpoint = match layout {
            BufferLayout::Linear => {
                let segment = segments
                    .first()
                    .ok_or_else(|| io::Error::other("linear span missing"))?;
                CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: segment.address,
                    offset: 0,
                })
            }
            BufferLayout::ScatterGather(_) => {
                let width = u64::try_from(Segment::RECORD_SIZE)?;
                let alignment =
                    MemoryAlignment::try_new(u64::try_from(Segment::RECORD_ALIGNMENT)?)?;
                let count = u64::try_from(segments.len())?;
                let size = count
                    .checked_mul(width)
                    .ok_or_else(|| io::Error::other("list size overflow"))?;
                let list = self.allocator.allocate(size, alignment)?;
                for (index, segment) in segments.iter().enumerate() {
                    let offset = u64::try_from(index)?
                        .checked_mul(width)
                        .ok_or_else(|| io::Error::other("list offset overflow"))?;
                    let address = list
                        .checked_add(offset)
                        .ok_or_else(|| io::Error::other("list address overflow"))?;
                    self.memory
                        .write(address, alignment, &segment.to_bytes()?)?;
                }
                CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                    list_address: list.raw(),
                    count,
                    index: 0,
                    offset: 0,
                })
            }
        };
        Ok(Buffer { endpoint, segments })
    }
}
