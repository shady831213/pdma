use pdma_model_contract::abi::mmio::{
    GlobalResponseBank, MmioResultDetails, ResponseUpdate, apply_response_update,
};

use super::image::RootImage;
use super::{invariant, narrow_u32};

#[derive(Clone, Copy)]
pub(crate) struct GlobalRequest {
    pub(crate) abi_version: u32,
    pub(crate) features: u64,
    pub(crate) reset_expected_epoch: u64,
}

pub(super) fn request(image: &RootImage) -> GlobalRequest {
    invariant(image.view(|root| {
        let global = invariant(root.global());
        GlobalRequest {
            abi_version: narrow_u32(invariant(invariant(global.req_abi_version()).value())),
            features: invariant(invariant(global.req_features()).value()),
            reset_expected_epoch: invariant(invariant(global.reset_expected_epoch()).value()),
        }
    }))
}

pub(super) fn complete(
    image: &RootImage,
    result: MmioResultDetails,
    update: ResponseUpdate<GlobalResponseBank>,
) {
    invariant(image.view_mut(|mut root| {
        let response = apply_response_update(read_response(&root), update);
        invariant(invariant(root.g_result_mut()).set_value(u64::from(result.result.code())));
        let [detail0, detail1, detail2] = result.details;
        invariant(invariant(root.g_detail0_mut()).set_value(detail0));
        invariant(invariant(root.g_detail1_mut()).set_value(detail1));
        invariant(invariant(root.g_detail2_mut()).set_value(detail2));
        write_response(&mut root, response);
    }));
}

fn read_response(
    root: &pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
) -> GlobalResponseBank {
    GlobalResponseBank {
        abi_version: narrow_u32(invariant(invariant(root.g_rsp_abi_version()).value())).into(),
        run_state: narrow_u32(invariant(invariant(root.g_rsp_run_state()).value())).into(),
        features: invariant(invariant(root.g_rsp_features()).value()).into(),
        epoch: invariant(invariant(root.g_rsp_epoch()).value()).into(),
        max_ring_depth: narrow_u32(invariant(invariant(root.g_max_ring_depth()).value())).into(),
        max_indirect_destinations: narrow_u32(invariant(
            invariant(root.g_max_indirect_destinations()).value(),
        ))
        .into(),
        max_transfer_bytes: narrow_u32(invariant(invariant(root.g_max_transfer_bytes()).value()))
            .into(),
        max_strided_rows: narrow_u32(invariant(invariant(root.g_max_strided_rows()).value()))
            .into(),
        max_stride: narrow_u32(invariant(invariant(root.g_max_stride()).value())).into(),
    }
}

fn write_response(
    root: &mut pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
    response: GlobalResponseBank,
) {
    invariant(invariant(root.g_rsp_abi_version_mut()).set_value(response.abi_version.raw().into()));
    invariant(invariant(root.g_rsp_run_state_mut()).set_value(response.run_state.raw().into()));
    invariant(invariant(root.g_rsp_features_mut()).set_value(response.features.raw()));
    invariant(invariant(root.g_rsp_epoch_mut()).set_value(response.epoch.raw()));
    invariant(
        invariant(root.g_max_ring_depth_mut()).set_value(response.max_ring_depth.raw().into()),
    );
    invariant(
        invariant(root.g_max_indirect_destinations_mut())
            .set_value(response.max_indirect_destinations.raw().into()),
    );
    invariant(
        invariant(root.g_max_transfer_bytes_mut())
            .set_value(response.max_transfer_bytes.raw().into()),
    );
    invariant(
        invariant(root.g_max_strided_rows_mut()).set_value(response.max_strided_rows.raw().into()),
    );
    invariant(invariant(root.g_max_stride_mut()).set_value(response.max_stride.raw().into()));
}
