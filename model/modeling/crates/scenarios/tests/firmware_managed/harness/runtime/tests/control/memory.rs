use super::super::AccessError;
use crate::harness::TestResult;
use crate::harness::runtime::platform::CONTROL_CHANNEL_BASE;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemoryAccess,
};
use hardware_interface_schema::Decode;
use pdma_model_contract::{ControlQueue, ControlQueueRaw, ControlRegisters, ControlRingState};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
};

pub(in super::super) struct QueueMemory<'a> {
    memory: &'a SharedMemoryAccess,
    state: MemoryAddress,
}

impl<'a> QueueMemory<'a> {
    pub(in super::super) const fn new(
        memory: &'a SharedMemoryAccess,
        state: MemoryAddress,
    ) -> Self {
        Self { memory, state }
    }

    pub(in super::super) fn read(&self) -> TestResult<ControlQueue> {
        Ok(self
            .memory
            .read_typed::<ControlQueueRaw>(self.state, MemoryAlignment::ONE)?
            .decode()?)
    }

    pub(in super::super) fn publish_command(&self, producer: u32) -> TestResult {
        self.write_cursor(
            ControlQueue::COMMAND_OFFSET + ControlRingState::PRODUCER_OFFSET,
            producer,
        )
    }

    pub(in super::super) fn consume_response(&self, consumer: u32) -> TestResult {
        self.write_cursor(
            ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
            consumer,
        )
    }

    fn write_cursor(&self, offset: usize, value: u32) -> TestResult {
        let address = self
            .state
            .raw()
            .checked_add(u64::try_from(offset)?)
            .ok_or("cursor overflow")?;
        self.memory.write(
            MemoryAddress::new(address),
            MemoryAlignment::ONE,
            &value.to_le_bytes(),
        )?;
        Ok(())
    }

    pub(in super::super) async fn wait_response(
        &self,
        interrupt: &pdma_model_software::control::InterruptEvent,
        producer: u32,
    ) -> TestResult {
        while self.read()?.response.producer != producer {
            interrupt.wait().await;
        }
        Ok(())
    }
}

pub(in super::super) async fn kick(
    bus: &(impl RegisterAccess<Error = AccessError> + Sync),
) -> TestResult {
    bus.write(
        CONTROL_CHANNEL_BASE + u32::try_from(ControlRegisters::REQUEST.offset())?,
        &1_u32.to_le_bytes(),
    )
    .await?;
    Ok(())
}

pub(in super::super) async fn poll_once<F: Future>(mut future: Pin<&mut F>) -> Poll<F::Output> {
    poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await
}
