mod progress;
#[cfg(test)]
mod tests;

use crate::{Dma, Error, Frontier, Maintenance};
use pdma_model_contract::dma::{ChannelId, ContextId};
use std::fmt;

/// A maintenance failure never authorizes context or memory reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceError {
    /// Another operation is still pending.
    Busy,
    /// The selector is still bound to a configured ring.
    RingBound(ChannelId),
    /// This channel cannot make progress; reset it before retrying cleanup.
    ChannelStopped(ChannelId),
    /// The requested channel does not exist.
    InvalidChannel(ChannelId),
    /// There is no retained result for this selector.
    Unknown,
    /// Snapshot storage could not be allocated.
    Allocation,
}

impl fmt::Display for MaintenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "context maintenance: {self:?}")
    }
}
impl std::error::Error for MaintenanceError {}

impl<M, S> Dma<M, S> {
    /// Captures published producers and waits for normal retirement of old work.
    pub fn drain_context(
        &mut self,
        context: ContextId,
        channels: &[ChannelId],
    ) -> Result<(), MaintenanceError> {
        self.begin_maintenance(context, channels, false)
    }

    /// Discards unstarted old target work; accepted commands finish normally.
    pub fn abort_context(
        &mut self,
        context: ContextId,
        channels: &[ChannelId],
    ) -> Result<(), MaintenanceError> {
        self.begin_maintenance(context, channels, true)
    }

    /// Reads the retained result; false means pending, not safe to release memory.
    pub fn context_status(&self, context: ContextId) -> Result<bool, MaintenanceError> {
        self.maintenance
            .as_ref()
            .filter(|m| m.context == context)
            .ok_or(MaintenanceError::Unknown)?
            .result
    }

    /// Channels detected as failed at the retained maintenance failure boundary.
    pub fn failed_context_channels(
        &self,
        context: ContextId,
    ) -> impl Iterator<Item = ChannelId> + '_ {
        self.maintenance
            .iter()
            .filter(move |m| m.context == context)
            .flat_map(|m| m.frontiers.iter())
            .filter(|f| f.failed)
            .map(|f| f.channel)
    }

    fn begin_maintenance(
        &mut self,
        context: ContextId,
        channels: &[ChannelId],
        abort: bool,
    ) -> Result<(), MaintenanceError> {
        if self
            .maintenance
            .as_ref()
            .is_some_and(|m| m.result == Ok(false))
        {
            return Err(MaintenanceError::Busy);
        }
        let mut frontiers = Vec::new();
        frontiers
            .try_reserve_exact(channels.len())
            .map_err(|_| MaintenanceError::Allocation)?;
        for &id in channels {
            let channel = self
                .channels
                .get(id.index())
                .ok_or(MaintenanceError::InvalidChannel(id))?;
            if channel.configured && channel.registers.values.ring_context.raw() == context.raw() {
                return Err(MaintenanceError::RingBound(id));
            }
            let pointers = channel.command.pointers();
            frontiers.push(Frontier {
                channel: id,
                generation: channel.generation,
                consumer: pointers.consumer,
                pending: pointers.pending(),
                passed: pointers.is_empty(),
                failed: false,
            });
        }
        self.maintenance = Some(Maintenance {
            context,
            abort,
            frontiers,
            result: Ok(false),
        });
        self.refresh_maintenance();
        Ok(())
    }

    pub(crate) fn maintenance_interlock(&self, id: Option<ChannelId>) -> Result<(), Error> {
        if self.maintenance.as_ref().is_some_and(|m| {
            m.result == Ok(false) && id.is_none_or(|id| m.frontiers.iter().any(|f| f.channel == id))
        }) {
            Err(Error::MaintenanceInProgress)
        } else {
            Ok(())
        }
    }
}
