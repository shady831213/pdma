use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_schema::Decode;
use pdma_model_contract::dma::{
    Endpoint, EndpointPayloadValues, ResponseStatus, Segment, SegmentRaw,
};

#[derive(Debug)]
pub(in crate::channel) struct Cursor {
    endpoint: Endpoint,
    segment: Option<Segment>,
    length: u64,
}

impl Cursor {
    pub(super) const fn new(endpoint: Endpoint, length: u64) -> Self {
        Self {
            endpoint,
            segment: None,
            length,
        }
    }

    pub(super) fn next_segment(
        &mut self,
        memory: &impl MemoryAccess,
    ) -> Result<bool, ResponseStatus> {
        let segment = match self.endpoint.payload {
            EndpointPayloadValues::Linear(linear) => Segment {
                address: linear.address,
                length: linear
                    .offset
                    .checked_add(self.length)
                    .ok_or(ResponseStatus::InvalidLength)?,
            },
            EndpointPayloadValues::ScatterGather(list) => {
                if list.index >= list.count {
                    return Ok(false);
                }
                let stride = u64::try_from(Segment::RECORD_SIZE)
                    .map_err(|_| ResponseStatus::InvalidLength)?;
                let address = list
                    .index
                    .checked_mul(stride)
                    .and_then(|offset| list.list_address.checked_add(offset))
                    .filter(|address| address.checked_add(stride).is_some())
                    .ok_or(ResponseStatus::InvalidLength)?;
                let alignment = MemoryAlignment::try_new(
                    u64::try_from(Segment::RECORD_ALIGNMENT)
                        .map_err(|_| ResponseStatus::InvalidLength)?,
                )
                .map_err(|_| ResponseStatus::InvalidLength)?;
                memory
                    .read_typed::<SegmentRaw>(MemoryAddress::new(address), alignment)
                    .map_err(|_| ResponseStatus::ListReadError)?
                    .decode()
                    .map_err(|_| ResponseStatus::ListReadError)?
            }
        };
        if self.offset() > segment.length {
            return Err(ResponseStatus::InvalidLength);
        }
        self.segment = Some(segment);
        self.advance(0);
        Ok(true)
    }

    const fn offset(&self) -> u64 {
        match self.endpoint.payload {
            EndpointPayloadValues::Linear(linear) => linear.offset,
            EndpointPayloadValues::ScatterGather(list) => list.offset,
        }
    }

    pub(super) fn remaining(&self) -> u64 {
        self.segment
            .map_or(0, |segment| segment.length - self.offset())
    }

    pub(super) fn address(&self) -> Option<u64> {
        self.segment?.address.checked_add(self.offset())
    }

    pub(super) fn advance(&mut self, count: u64) {
        match &mut self.endpoint.payload {
            EndpointPayloadValues::Linear(linear) => linear.offset += count,
            EndpointPayloadValues::ScatterGather(list) => {
                list.offset += count;
                if self
                    .segment
                    .is_some_and(|segment| list.offset == segment.length)
                {
                    list.index += 1;
                    list.offset = 0;
                    self.segment = None;
                }
            }
        }
    }
}
