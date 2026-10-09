use super::memory::{PageError, Paged};
use crate::harness::runtime::platform::MemoryRequest;
use hardware_interface_runtime::{BoundMemory, MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{
    dma::ChannelId,
    memory::{FunctionId, MemoryIdentity},
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
/// Opaque platform source identity, stable for one runtime instance and physical channel.
pub struct FaultTag(Arc<()>);

impl PartialEq for FaultTag {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for FaultTag {}

#[derive(Clone, Debug)]
pub(in crate::harness) struct Sources(Arc<[FaultTag]>);

impl Sources {
    pub(in crate::harness) fn new(count: usize) -> Self {
        Self((0..count).map(|_| FaultTag(Arc::new(()))).collect())
    }

    pub(in crate::harness) fn tag(&self, channel: ChannelId) -> Option<FaultTag> {
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
                .map(|channel| self.tag(*channel).map(|tag| (*channel, tag)))
                .collect::<Option<_>>()?,
        })
    }
}

pub(in crate::harness) struct FaultHandler {
    function: FunctionId,
    sources: Vec<(ChannelId, FaultTag)>,
}

impl FaultHandler {
    pub(in crate::harness) fn resolve(&self, report: &super::FaultReport) -> Option<ChannelId> {
        if report.identity.function != self.function {
            return None;
        }
        self.sources
            .iter()
            .find(|(_, tag)| *tag == report.tag)
            .map(|(channel, _)| *channel)
    }

    pub(in crate::harness) fn retire(&mut self) {
        self.sources.clear();
    }
}

#[derive(Clone, Debug)]
pub(super) struct TaggedRequest {
    pub identity: MemoryIdentity,
    pub tag: Option<FaultTag>,
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
                tag: None,
            },
        }
    }

    pub(in crate::harness) fn bind(&self, request: MemoryRequest) -> Self {
        Self {
            memory: Arc::clone(&self.memory),
            sources: self.sources.clone(),
            request: TaggedRequest {
                identity: request.identity,
                tag: request
                    .channel
                    .and_then(|channel| self.sources.tag(channel)),
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
