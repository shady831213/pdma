use hardware_interface_runtime::{
    RegisterBusAddress, RegisterBusBase, RegisterBusOffset, RegisterBusWidth, RegisterImage,
};
use pdma_model_contract::abi::mmio::{
    IrqResponseBank, MmioResultDetails, PdmaMmioMap, PdmaMmioMapView, PdmaMmioMapViewMut,
    QueueResponseBank, RegisterId, ResponseUpdate,
};

use super::irq::IrqRequest;
use super::queue::QueueStaging;
use super::storage::DeviceRegisterStorage;
use super::{invariant, irq, queue};
use crate::interface::mmio::{BootstrapBatchStaging, SubmissionBatchStaging};

pub(super) type RootImage = RegisterImage<PdmaMmioMap, DeviceRegisterStorage>;

#[derive(Debug)]
pub(crate) struct PdmaRegisters {
    image: RootImage,
}

impl PdmaRegisters {
    pub(crate) fn new() -> Self {
        Self {
            image: invariant(RegisterImage::new()),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.image = invariant(RegisterImage::new());
    }

    pub(crate) fn read(&self, register: RegisterId) -> Vec<u8> {
        let definition = invariant(PdmaMmioMap::register(register).ok_or(()));
        let width = definition.definition().width();
        let mut bytes = vec![0; width];
        invariant(
            self.image
                .read_leaf(offset(definition.offset(), width), &mut bytes),
        );
        bytes
    }

    pub(crate) fn write(&self, register: RegisterId, bytes: &[u8]) {
        let definition = invariant(PdmaMmioMap::register(register).ok_or(()));
        invariant(
            self.image
                .write_leaf(offset(definition.offset(), bytes.len()), bytes),
        );
    }

    pub(crate) fn snapshot_queue(&self) -> QueueStaging {
        queue::staging(&self.image)
    }

    pub(crate) fn snapshot_irq(&self) -> IrqRequest {
        irq::request(&self.image)
    }

    pub(crate) fn complete_queue(
        &self,
        result: MmioResultDetails,
        update: ResponseUpdate<QueueResponseBank>,
    ) {
        queue::complete(&self.image, result, update);
    }

    pub(crate) fn complete_irq(
        &self,
        result: MmioResultDetails,
        update: ResponseUpdate<IrqResponseBank>,
    ) {
        irq::complete(&self.image, result, update);
    }

    pub(crate) fn complete_bootstrap_batch(
        &self,
        staging: BootstrapBatchStaging,
        response: QueueResponseBank,
    ) {
        queue::complete_bootstrap_batch(&self.image, staging, response);
    }

    pub(crate) fn complete_submission_batch(
        &self,
        staging: SubmissionBatchStaging,
        response: QueueResponseBank,
    ) {
        queue::complete_submission_batch(&self.image, staging, response);
    }

    pub(crate) fn view<R>(
        &self,
        operation: impl for<'view> FnOnce(PdmaMmioMapView<'view>) -> R,
    ) -> R {
        invariant(self.image.view(operation))
    }

    pub(crate) fn view_mut<R>(
        &self,
        operation: impl for<'view> FnOnce(PdmaMmioMapViewMut<'view>) -> R,
    ) -> R {
        invariant(self.image.view_mut(operation))
    }

    #[cfg(test)]
    pub(crate) fn image_extent(&self) -> usize {
        invariant(self.image.view(|_| PdmaMmioMap::EXTENT))
    }

    #[cfg(debug_assertions)]
    pub(crate) fn inspect_for_test<R>(
        &self,
        operation: impl for<'view> FnOnce(PdmaMmioMapView<'view>) -> R,
    ) -> R {
        self.view(operation)
    }

    #[cfg(debug_assertions)]
    pub(crate) fn mutate_for_test<R>(
        &self,
        operation: impl for<'view> FnOnce(PdmaMmioMapViewMut<'view>) -> R,
    ) -> R {
        self.view_mut(operation)
    }
}

fn offset(raw: usize, width: usize) -> RegisterBusOffset {
    invariant(RegisterBusOffset::from_address(
        RegisterBusBase::new(0),
        RegisterBusAddress::new(invariant(u64::try_from(raw))),
        RegisterBusWidth::new(width),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdma_model_contract::abi::mmio::{GlobalResponseBank, MmioResult};

    #[test]
    fn root_image_is_the_only_storage_for_typed_and_raw_access() {
        let before = super::super::allocation_observation();
        let registers = PdmaRegisters::new();
        assert_eq!(
            super::super::allocation_observation(),
            (before.0 + 1, before.1 + 1)
        );
        registers.view_mut(|mut root| {
            let mut global = invariant(root.global_mut());
            invariant(invariant(global.req_abi_version_mut()).set_value(0x1234));
        });
        assert_eq!(
            registers.view(|root| {
                let global = invariant(root.global());
                invariant(invariant(global.req_abi_version()).value())
            }),
            0x1234
        );
        assert_eq!(
            registers.read(RegisterId::GReqAbiVersion),
            0x1234_u32.to_le_bytes()
        );
        let response = GlobalResponseBank {
            abi_version: 1.into(),
            run_state: 2.into(),
            features: 3.into(),
            epoch: 4.into(),
            max_ring_depth: 5.into(),
            max_indirect_destinations: 6.into(),
            max_transfer_bytes: 7.into(),
            max_strided_rows: 8.into(),
            max_stride: 9.into(),
        };
        registers.view_mut(|mut root| {
            let mut global = invariant(root.global_mut());
            invariant(invariant(global.result_mut()).set_value(u64::from(MmioResult::Ok.code())));
            invariant(invariant(global.rsp_abi_version_mut()).set_value(1));
            invariant(invariant(global.rsp_run_state_mut()).set_value(2));
            invariant(invariant(global.rsp_features_mut()).set_value(3));
            invariant(invariant(global.rsp_epoch_mut()).set_value(4));
            invariant(invariant(global.max_ring_depth_mut()).set_value(5));
            invariant(invariant(global.max_indirect_destinations_mut()).set_value(6));
            invariant(invariant(global.max_transfer_bytes_mut()).set_value(7));
            invariant(invariant(global.max_strided_rows_mut()).set_value(8));
            invariant(invariant(global.max_stride_mut()).set_value(9));
        });
        let raw: Vec<u8> = [
            RegisterId::GRspAbiVersion,
            RegisterId::GRspRunState,
            RegisterId::GRspFeatures,
            RegisterId::GRspEpoch,
            RegisterId::GMaxRingDepth,
            RegisterId::GMaxIndirectDestinations,
            RegisterId::GMaxTransferBytes,
            RegisterId::GMaxStridedRows,
            RegisterId::GMaxStride,
        ]
        .into_iter()
        .flat_map(|register| registers.read(register))
        .collect();
        assert_eq!(raw, invariant(response.to_bytes()).as_slice());
        assert_eq!(registers.image_extent(), 4096);
        let duplicate = PdmaRegisters::new();
        assert_eq!(
            super::super::allocation_observation(),
            (before.0 + 2, before.1 + 2)
        );
        assert_eq!(duplicate.image_extent(), 4096);
        drop(duplicate);
        assert_eq!(
            super::super::allocation_observation(),
            (before.0 + 2, before.1 + 1)
        );
    }
}
