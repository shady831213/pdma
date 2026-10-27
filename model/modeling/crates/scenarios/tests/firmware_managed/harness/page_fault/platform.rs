use super::memory::{PageError, Paged};
use crate::harness::runtime::platform::MemoryRequest;
use hardware_interface_io::memory::{BoundMemory, MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{
    dma::ChannelId,
    memory::{FunctionId, MemoryIdentity},
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
/// Opaque platform source identity, stable for one runtime instance and physical channel.
pub struct ChannelTag(Arc<()>);

impl PartialEq for ChannelTag {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ChannelTag {}

#[derive(Clone, Debug)]
pub(in crate::harness) struct Sources(Arc<[ChannelTag]>);

impl Sources {
    pub(in crate::harness) fn new(count: usize) -> Self {
        Self((0..count).map(|_| ChannelTag(Arc::new(()))).collect())
    }

    pub(in crate::harness) fn ch_tag(&self, channel: ChannelId) -> Option<ChannelTag> {
        self.0.get(channel.index()).cloned()
    }

    pub(in crate::harness) fn handler(
        &self,
        function: FunctionId,
        channels: &[ChannelId],
    ) -> Option<FaultHandler> {
        Some(FaultHandler {
            function,
            sources: channels
                .iter()
                .map(|channel| self.ch_tag(*channel).map(|ch_tag| (*channel, ch_tag)))
                .collect::<Option<_>>()?,
        })
    }
}

pub(in crate::harness) struct FaultHandler {
    function: FunctionId,
    sources: Vec<(ChannelId, ChannelTag)>,
}

impl FaultHandler {
    pub(in crate::harness) fn resolve(&self, report: &super::FaultReport) -> Option<ChannelId> {
        if report.identity.function != self.function {
            return None;
        }
        self.sources
            .iter()
            .find(|(_, ch_tag)| *ch_tag == report.ch_tag)
            .map(|(channel, _)| *channel)
    }

    pub(in crate::harness) fn retire(&mut self) {
        self.sources.clear();
    }
}

#[derive(Clone, Debug)]
pub(super) struct TaggedRequest {
    pub identity: MemoryIdentity,
    pub ch_tag: Option<ChannelTag>,
}

#[derive(Clone, Debug)]
pub(in crate::harness) struct Backend {
    memory: Arc<Paged>,
    sources: Sources,
    request: TaggedRequest,
}

impl Backend {
    pub(super) fn new(memory: Paged, sources: Sources, identity: MemoryIdentity) -> Self {
        Self {
            memory: Arc::new(memory),
            sources,
            request: TaggedRequest {
                identity,
                ch_tag: None,
            },
        }
    }

    pub(in crate::harness) fn bind(&self, request: MemoryRequest) -> Self {
        Self {
            memory: Arc::clone(&self.memory),
            sources: self.sources.clone(),
            request: TaggedRequest {
                identity: request.identity,
                ch_tag: request
                    .channel
                    .and_then(|channel| self.sources.ch_tag(channel)),
            },
        }
    }
}

impl MemoryAccess for Backend {
    type Error = PageError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        BoundMemory::new(self.memory.as_ref(), self.request.clone())
            .validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        BoundMemory::new(self.memory.as_ref(), self.request.clone()).read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        BoundMemory::new(self.memory.as_ref(), self.request.clone())
            .write(address, alignment, bytes)
    }
}
