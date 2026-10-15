use crate::channel::Continuation;
use crate::{Channel, Dma, Error, Work};
use hardware_interface_runtime::BoundMemory;
use pdma_model_contract::dma::{ChannelId, ContextId, DmaAccess};

/// Borrowed execution resources, excluding device MMIO, arbitration and maintenance.
#[derive(Debug)]
pub struct ExecutionContext<'a, M, S = ()> {
    pub(crate) selected: &'a mut Channel,
    pub(crate) id: ChannelId,
    pub(crate) memory: &'a M,
    pub(crate) sideband: fn(DmaAccess) -> S,
    pub(crate) halted: bool,
    pub(crate) active: &'a mut Option<Work>,
}

impl<M, S> Dma<M, S> {
    /// Borrows only resources used by descriptor, payload and publication stages.
    pub fn execution_context(
        &mut self,
        id: ChannelId,
    ) -> Result<ExecutionContext<'_, M, S>, Error> {
        let halted = self.halted() || self.resetting();
        Ok(ExecutionContext {
            selected: self.channels.get_mut(id.index()).ok_or(Error::Unmapped)?,
            id,
            memory: &self.memory,
            sideband: self.sideband,
            halted,
            active: &mut self.active,
        })
    }
}

impl<M, S> ExecutionContext<'_, M, S> {
    /// Installs validated traversal once and returns its first chunk request.
    pub fn install_progress(
        &mut self,
        validated: crate::CopyProgress,
    ) -> Result<crate::ChunkInput, Error> {
        let channel = self.channel(validated.work)?;
        let accepted = channel.work.as_mut().ok_or(Error::Reset)?;
        accepted.progress = Some(validated.progress);
        Ok(crate::ChunkInput::next(validated.work))
    }
    pub(crate) fn channel(&mut self, work: Work) -> Result<&mut Channel, Error> {
        if work.channel != self.id {
            return Err(Error::Routing);
        }
        let channel = &mut *self.selected;
        if channel.generation != work.generation || channel.resetting() {
            return Err(Error::Reset);
        }
        if self.halted || channel.halted() {
            return Err(Error::Halted);
        }
        Ok(channel)
    }

    pub(crate) fn ready(&mut self, work: Work) -> Result<bool, Error> {
        let channel = self.channel(work)?;
        if channel.work.is_none() {
            return Err(Error::Reset);
        }
        Ok(channel.enabled() && !channel.registers.values.fault_status.faulted())
    }

    pub(crate) fn memory_for(&self, work: Work, selector: u32) -> BoundMemory<'_, M, S> {
        BoundMemory::new(
            self.memory,
            (self.sideband)(DmaAccess {
                channel: work.channel,
                context: ContextId::new(selector),
            }),
        )
    }

    pub(crate) fn retain(&mut self, work: Work, continuation: Continuation) -> Result<(), Error> {
        self.channel(work)?
            .work
            .as_mut()
            .ok_or(Error::Reset)?
            .continuation = Some(continuation);
        Ok(())
    }

    pub(crate) fn park(&mut self, work: Work, continuation: Continuation) -> Result<(), Error> {
        self.retain(work, continuation)?;
        self.channel(work)?.suspend();
        if *self.active == Some(work) {
            *self.active = None;
        }
        Ok(())
    }
}
