use super::{Management, ManagementRequestId};
use crate::Error;
use hardware_interface_io::memory::MemoryAccess;
use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::control::{ControlRequest, ControlRequestOpcode};
use pdma_model_contract::dma::ContextId;
use pdma_model_contract::memory::{ContextAttributes, FunctionId};

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Publishes binding creation; collect its selector through `next_response`.
    pub async fn submit_create_context(
        &mut self,
        function: FunctionId,
        attributes: ContextAttributes,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit_request(ControlRequest {
            id: 0,
            opcode: ControlRequestOpcode::CreateContext,
            function: function.raw(),
            channel: 0,
            context: 0,
            attributes,
        })
        .await
    }

    /// Requests drain after submitters stop publishing references to this selector.
    pub async fn submit_drain_context(
        &mut self,
        function: FunctionId,
        context: ContextId,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit_context(function, (ControlRequestOpcode::DrainContext, context))
            .await
    }

    /// Requests collective retirement; retain bindings and storage until success.
    pub async fn submit_abort_context(
        &mut self,
        function: FunctionId,
        context: ContextId,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit_context(function, (ControlRequestOpcode::AbortContext, context))
            .await
    }

    /// Requests drain and release; keep submission exclusion through successful completion.
    pub async fn submit_destroy_context(
        &mut self,
        function: FunctionId,
        context: ContextId,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit_context(function, (ControlRequestOpcode::DestroyContext, context))
            .await
    }

    async fn submit_context(
        &mut self,
        function: FunctionId,
        (opcode, context): (ControlRequestOpcode, ContextId),
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit_request(ControlRequest {
            id: 0,
            opcode,
            function: function.raw(),
            channel: 0,
            context: context.raw(),
            attributes: ContextAttributes::DEFAULT,
        })
        .await
    }
}
