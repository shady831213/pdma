use pdma_model_contract::abi::mmio::{
    IrqResponseBank, MmioResultDetails, ResponseUpdate, apply_response_update,
};

use super::image::RootImage;
use super::{invariant, narrow_u32};

#[derive(Clone, Copy)]
pub(crate) struct IrqRequest {
    pub(crate) expected_epoch: u64,
    pub(crate) source_id: u64,
    pub(crate) source_kind: u32,
    pub(crate) route_kind: u32,
    pub(crate) route_vector: u64,
}

pub(super) fn request(image: &RootImage) -> IrqRequest {
    invariant(image.view(|root| {
        let irq = invariant(root.irq());
        IrqRequest {
            expected_epoch: invariant(invariant(irq.expected_epoch()).value()),
            source_id: invariant(invariant(irq.source_id()).value()),
            source_kind: narrow_u32(invariant(invariant(irq.source_kind()).value())),
            route_kind: narrow_u32(invariant(invariant(irq.route_kind()).value())),
            route_vector: invariant(invariant(irq.route_vector()).value()),
        }
    }))
}

pub(super) fn complete(
    image: &RootImage,
    result: MmioResultDetails,
    update: ResponseUpdate<IrqResponseBank>,
) {
    invariant(image.view_mut(|mut root| {
        let response = apply_response_update(read_response(&root), update);
        invariant(invariant(root.i_result_mut()).set_value(u64::from(result.result.code())));
        let [detail0, detail1, detail2] = result.details;
        invariant(invariant(root.i_detail0_mut()).set_value(detail0));
        invariant(invariant(root.i_detail1_mut()).set_value(detail1));
        invariant(invariant(root.i_detail2_mut()).set_value(detail2));
        write_response(&mut root, response);
    }));
}

fn read_response(root: &pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>) -> IrqResponseBank {
    IrqResponseBank {
        epoch: invariant(invariant(root.i_rsp_epoch()).value()).into(),
        source_id: invariant(invariant(root.i_rsp_source_id()).value()).into(),
        source_kind: narrow_u32(invariant(invariant(root.i_rsp_source_kind()).value())).into(),
        state: narrow_u32(invariant(invariant(root.i_rsp_state()).value())).into(),
        route_kind: narrow_u32(invariant(invariant(root.i_rsp_route_kind()).value())).into(),
        notification_kind: narrow_u32(invariant(invariant(root.i_rsp_notification_kind()).value()))
            .into(),
        vector: invariant(invariant(root.i_rsp_vector()).value()).into(),
        message_id: invariant(invariant(root.i_rsp_message_id()).value()).into(),
        pending_messages: invariant(invariant(root.i_rsp_pending_messages()).value()).into(),
    }
}

fn write_response(
    root: &mut pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
    response: IrqResponseBank,
) {
    invariant(invariant(root.i_rsp_epoch_mut()).set_value(response.epoch.raw()));
    invariant(invariant(root.i_rsp_source_id_mut()).set_value(response.source_id.raw()));
    invariant(invariant(root.i_rsp_source_kind_mut()).set_value(response.source_kind.raw().into()));
    invariant(invariant(root.i_rsp_state_mut()).set_value(response.state.raw().into()));
    invariant(invariant(root.i_rsp_route_kind_mut()).set_value(response.route_kind.raw().into()));
    invariant(
        invariant(root.i_rsp_notification_kind_mut())
            .set_value(response.notification_kind.raw().into()),
    );
    invariant(invariant(root.i_rsp_vector_mut()).set_value(response.vector.raw()));
    invariant(invariant(root.i_rsp_message_id_mut()).set_value(response.message_id.raw()));
    invariant(
        invariant(root.i_rsp_pending_messages_mut()).set_value(response.pending_messages.raw()),
    );
}
