use super::{AsyncDriver, ManualDriver, core::DriverCore};
use crate::AsyncDevicePort;
use crate::ModelResult;
use pdma_model_contract::{
    BroadcastCopyCommand, BroadcastCopyRequest, CommandId, CopyCommand, CopyRequest,
    ExternalCommand, StridedCopyCommand, StridedCopyRequest, VectorAddCommand, VectorAddRequest,
};

impl DriverCore {
    async fn submit_copy_on(
        &self,
        device: &mut impl AsyncDevicePort,
        request: CopyRequest,
    ) -> ModelResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = CopyCommand::new(
            reservation.command_id(),
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, reservation, ExternalCommand::Copy(command))
            .await
    }

    async fn submit_strided_copy_on(
        &self,
        device: &mut impl AsyncDevicePort,
        request: StridedCopyRequest,
    ) -> ModelResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = StridedCopyCommand::new(
            reservation.command_id(),
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, reservation, ExternalCommand::StridedCopy(command))
            .await
    }

    async fn submit_broadcast_copy_on(
        &self,
        device: &mut impl AsyncDevicePort,
        request: BroadcastCopyRequest,
    ) -> ModelResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = BroadcastCopyCommand::new(
            reservation.command_id(),
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(
            device,
            reservation,
            ExternalCommand::BroadcastCopy(command),
        )
        .await
    }

    async fn submit_vector_add_on(
        &self,
        device: &mut impl AsyncDevicePort,
        request: VectorAddRequest,
    ) -> ModelResult<CommandId> {
        let reservation = self.accounting.prepare()?;
        let command = VectorAddCommand::new(
            reservation.command_id(),
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, reservation, ExternalCommand::VectorAdd(command))
            .await
    }

    async fn submit_external_command_on(
        &self,
        device: &mut impl AsyncDevicePort,
        reservation: super::accounting::CommandReservation,
        command: ExternalCommand,
    ) -> ModelResult<CommandId> {
        let command_id = reservation.command_id();
        self.accounting
            .poll_valid(device.submit_command(self.queue_config, command))
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
                device: &mut impl AsyncDevicePort,
                request: CopyRequest,
            ) -> ModelResult<CommandId> {
                self.core.submit_copy_on(device, request).await
            }

            /// Emits a strided copy descriptor through an async device port and rings the doorbell.
            pub async fn submit_strided_copy_on(
                &mut self,
                device: &mut impl AsyncDevicePort,
                request: StridedCopyRequest,
            ) -> ModelResult<CommandId> {
                self.core.submit_strided_copy_on(device, request).await
            }

            /// Emits a broadcast copy descriptor through an async port and rings the doorbell.
            pub async fn submit_broadcast_copy_on(
                &mut self,
                device: &mut impl AsyncDevicePort,
                request: BroadcastCopyRequest,
            ) -> ModelResult<CommandId> {
                self.core.submit_broadcast_copy_on(device, request).await
            }

            /// Emits a vector-add descriptor through an async device port and rings the doorbell.
            pub async fn submit_vector_add_on(
                &mut self,
                device: &mut impl AsyncDevicePort,
                request: VectorAddRequest,
            ) -> ModelResult<CommandId> {
                self.core.submit_vector_add_on(device, request).await
            }
        }
    };
}

impl_driver_submission!(AsyncDriver);
impl_driver_submission!(ManualDriver);
