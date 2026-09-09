use crate::Error;
use hardware_interface_runtime::{CpuOwner, RegisterAccess};
use std::{
    future::{Future, poll_fn},
    pin::pin,
    task::Poll,
};

#[derive(Debug)]
pub(super) struct RegisterPort<B> {
    bus: B,
    owner: CpuOwner,
}

impl<B: RegisterAccess + Send + Sync> RegisterPort<B> {
    pub(super) const fn new(bus: B, owner: CpuOwner) -> Self {
        Self { bus, owner }
    }

    pub(super) fn validate_active(&self) -> Result<(), Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)
    }

    pub(super) async fn read_word(&self, offset: usize) -> Result<u32, Error<B::Error>> {
        self.validate_active()?;
        let mut bytes = [0; 4];
        self.checked_bus(self.bus.read(
            u32::try_from(offset).map_err(|_| Error::Geometry)?,
            &mut bytes,
        ))
        .await?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn write(&self, offset: usize, bytes: &[u8]) -> Result<(), Error<B::Error>> {
        self.validate_active()?;
        self.checked_bus(
            self.bus
                .write(u32::try_from(offset).map_err(|_| Error::Geometry)?, bytes),
        )
        .await
    }

    async fn checked_bus(
        &self,
        operation: impl Future<Output = Result<(), B::Error>> + Send,
    ) -> Result<(), Error<B::Error>> {
        let mut operation = pin!(operation);
        poll_fn(|context| {
            if let Err(error) = self.owner.validate_active() {
                return Poll::Ready(Err(Error::Cpu(error)));
            }
            operation.as_mut().poll(context).map_err(Error::Bus)
        })
        .await
    }
}
