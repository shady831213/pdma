use super::{Management, Pending};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{
    ControlRegisters, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseRaw,
    ControlRingRegisters, Ring, RingPointers,
};

const COMMAND: usize = ControlRegisters::COMMAND.offset();
const RESPONSE: usize = ControlRegisters::RESPONSE.offset();

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    pub(super) async fn begin(
        &mut self,
        opcode: ControlRequestOpcode,
        channel: u32,
    ) -> Result<(), Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)?;
        if self.pending.is_some() {
            return Err(Error::ManagementBusy);
        }
        let command = self.pointers(COMMAND).await?;
        let response = self.pointers(RESPONSE).await?;
        if !command.is_empty() || !response.is_empty() {
            return Err(Error::ManagementProtocol);
        }
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(Error::ManagementProtocol)?;
        self.pending = Some(Pending {
            request: ControlRequest {
                id,
                opcode,
                channel,
            },
            producer: command.producer,
            consumer: response.consumer,
            response: None,
        });
        Ok(())
    }

    pub(super) async fn collect(&mut self) -> Result<ControlResponse, Error<B::Error>> {
        let mut pending = self.pending.ok_or(Error::ManagementIdle)?;
        let producer = self
            .read(COMMAND + ControlRingRegisters::PRODUCER.offset())
            .await?;
        if producer == pending.producer {
            let pointers = self.pointers(COMMAND).await?;
            let next = self
                .rings
                .command
                .advance_producer(pointers, 1)
                .map_err(Error::Ring)?;
            let address =
                MemoryAddress::new(self.rings.command.entry(producer).map_err(Error::Ring)?);
            self.memory
                .write_typed(
                    address,
                    MemoryAlignment::ONE,
                    &pending.request.encode().map_err(Error::Codec)?,
                )
                .map_err(Error::Memory)?;
            self.write(
                COMMAND + ControlRingRegisters::PRODUCER.offset(),
                &next.producer.to_le_bytes(),
            )
            .await?;
        } else if producer != pending.producer.wrapping_add(1) {
            return Err(Error::ManagementProtocol);
        }
        let response = match pending.response {
            Some(response) => response,
            None => loop {
                let producer = self
                    .read(RESPONSE + ControlRingRegisters::PRODUCER.offset())
                    .await?;
                let pointers = RingPointers {
                    producer,
                    consumer: pending.consumer,
                };
                if pointers.is_empty() {
                    tokio::task::yield_now().await;
                    continue;
                }
                self.rings
                    .response
                    .advance_consumer(pointers, 1)
                    .map_err(Error::Ring)?;
                let address = MemoryAddress::new(
                    self.rings
                        .response
                        .entry(pending.consumer)
                        .map_err(Error::Ring)?,
                );
                let response = self
                    .memory
                    .read_typed::<ControlResponseRaw>(address, MemoryAlignment::ONE)
                    .map_err(Error::Memory)?
                    .decode()
                    .map_err(Error::Codec)?;
                if response.id != pending.request.id {
                    return Err(Error::ManagementProtocol);
                }
                pending.response = Some(response);
                self.pending = Some(pending);
                break response;
            },
        };
        let consumer = self
            .read(RESPONSE + ControlRingRegisters::CONSUMER.offset())
            .await?;
        if consumer == pending.consumer {
            self.write(
                RESPONSE + ControlRingRegisters::CONSUMER.offset(),
                &consumer.wrapping_add(1).to_le_bytes(),
            )
            .await?;
        } else if consumer != pending.consumer.wrapping_add(1) {
            return Err(Error::ManagementProtocol);
        }
        self.pending = None;
        Ok(response)
    }
}
