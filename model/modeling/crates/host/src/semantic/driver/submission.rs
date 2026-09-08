use super::{AsyncDriver, ManualDriver, core::DriverCore};
use crate::DriverPort;
use crate::DriverResult;
use crate::{
    BroadcastCopyRequest, Command, CommandOperation, CopyRequest, StridedCopyRequest,
    VectorAddRequest,
};
use pdma_model_contract::semantic::CommandId;

impl DriverCore {
    async fn submit_copy_on(
        &self,
        device: &mut impl DriverPort,
        request: CopyRequest,
    ) -> DriverResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = Command::new(reservation.command_id(), CommandOperation::Copy(request));
        self.submit_command_on(device, reservation, command).await
    }

    async fn submit_strided_copy_on(
        &self,
        device: &mut impl DriverPort,
        request: StridedCopyRequest,
    ) -> DriverResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = Command::new(
            reservation.command_id(),
            CommandOperation::StridedCopy(request),
        );
        self.submit_command_on(device, reservation, command).await
    }

    async fn submit_broadcast_copy_on(
        &self,
        device: &mut impl DriverPort,
        request: BroadcastCopyRequest,
    ) -> DriverResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = Command::new(
            reservation.command_id(),
            CommandOperation::BroadcastCopy(request),
        );
        self.submit_command_on(device, reservation, command).await
    }

    async fn submit_vector_add_on(
        &self,
        device: &mut impl DriverPort,
        request: VectorAddRequest,
    ) -> DriverResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = Command::new(
            reservation.command_id(),
            CommandOperation::VectorAdd(request),
        );
        self.submit_command_on(device, reservation, command).await
    }

    async fn submit_command_on(
        &self,
        device: &mut impl DriverPort,
        reservation: super::accounting::CommandReservation,
        command: Command,
    ) -> DriverResult<CommandId> {
        let command_id = reservation.command_id();
        self.accounting
            .poll_valid(device.submit_command_tracked(
                self.queue_config,
                command,
                reservation.receipt(),
            ))
            .await?;
        // Acceptance transfers rollback ownership from the local reservation.
        reservation.disarm();
        self.accounting
            .poll_valid(device.ring_doorbell(self.queue_config))
            .await?;
        Ok(command_id)
    }
}

macro_rules! impl_driver_submission {
    ($driver:ty) => {
        impl $driver {
            /// Emits a copy descriptor through an async device port and rings the doorbell.
            pub async fn submit_copy_on(
                &mut self,
                device: &mut impl DriverPort,
                request: CopyRequest,
            ) -> DriverResult<CommandId> {
                self.core.submit_copy_on(device, request).await
            }

            /// Emits a strided copy descriptor through an async device port and rings the doorbell.
            pub async fn submit_strided_copy_on(
                &mut self,
                device: &mut impl DriverPort,
                request: StridedCopyRequest,
            ) -> DriverResult<CommandId> {
                self.core.submit_strided_copy_on(device, request).await
            }

            /// Emits a broadcast copy descriptor through an async port and rings the doorbell.
            pub async fn submit_broadcast_copy_on(
                &mut self,
                device: &mut impl DriverPort,
                request: BroadcastCopyRequest,
            ) -> DriverResult<CommandId> {
                self.core.submit_broadcast_copy_on(device, request).await
            }

            /// Emits a vector-add descriptor through an async device port and rings the doorbell.
            pub async fn submit_vector_add_on(
                &mut self,
                device: &mut impl DriverPort,
                request: VectorAddRequest,
            ) -> DriverResult<CommandId> {
                self.core.submit_vector_add_on(device, request).await
            }
        }
    };
}

impl_driver_submission!(AsyncDriver);
impl_driver_submission!(ManualDriver);
