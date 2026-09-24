use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_schema::Decode;
use pdma_model_contract::dma::{
    Endpoint, EndpointPayloadValues, ResponseStatus, Segment, SegmentRaw,
};

#[derive(Debug)]
pub(super) struct Cursor {
    endpoint: Endpoint,
    index: u64,
    segment: Segment,
    offset: u64,
}

impl Cursor {
    pub(super) const fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            index: 0,
            segment: Segment {
                address: 0,
                length: 0,
            },
            offset: 0,
        }
    }

    pub(super) fn next_segment(
        &mut self,
        memory: &impl MemoryAccess,
    ) -> Result<Option<Segment>, ResponseStatus> {
        let segment = match self.endpoint.payload {
            EndpointPayloadValues::Linear(linear) => {
                if self.index != 0 {
                    return Ok(None);
                }
                Segment {
                    address: linear.address,
                    length: linear.length,
                }
            }
            EndpointPayloadValues::ScatterGather(list) => {
                if self.index == list.count {
                    return Ok(None);
                }
                let stride = u64::try_from(Segment::RECORD_SIZE)
                    .map_err(|_| ResponseStatus::InvalidLength)?;
                list.count
                    .checked_mul(stride)
                    .and_then(|size| list.list_address.checked_add(size))
                    .ok_or(ResponseStatus::InvalidLength)?;
                let address = self
                    .index
                    .checked_mul(stride)
                    .and_then(|offset| list.list_address.checked_add(offset))
                    .ok_or(ResponseStatus::InvalidLength)?;
                let alignment = MemoryAlignment::try_new(
                    u64::try_from(Segment::RECORD_ALIGNMENT)
                        .map_err(|_| ResponseStatus::InvalidLength)?,
                )
                .map_err(|_| ResponseStatus::InvalidLength)?;
                memory
                    .read_typed::<SegmentRaw>(MemoryAddress::new(address), alignment)
                    .map_err(|_| ResponseStatus::ListReadFault)?
                    .decode()
                    .map_err(|_| ResponseStatus::ListReadFault)?
            }
        };
        self.index += 1;
        self.segment = segment;
        self.offset = 0;
        Ok(Some(segment))
    }

    pub(super) const fn remaining(&self) -> u64 {
        self.segment.length - self.offset
    }
    pub(super) const fn address(&self) -> Option<u64> {
        self.segment.address.checked_add(self.offset)
    }
    pub(super) const fn advance(&mut self, count: u64) {
        self.offset += count;
    }
}
