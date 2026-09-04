use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueResponseBank, ResponseUpdate, apply_response_update,
};

use super::image::RootImage;
use super::{invariant, narrow_u32};
use crate::interface::mmio::{BootstrapBatchStaging, SubmissionBatchStaging};

#[derive(Clone, Copy)]
pub(crate) struct QueueStaging {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) expected_cursor: u64,
    pub(crate) new_cursor: u64,
    pub(crate) cfg_sq_depth: u32,
    pub(crate) cfg_cq_depth: u32,
    pub(crate) cfg_software_owner: u64,
    pub(crate) sq_base: u64,
    pub(crate) sq_depth: u32,
    pub(crate) cq_base: u64,
    pub(crate) cq_depth: u32,
    pub(crate) eq_base: u64,
    pub(crate) eq_depth: u32,
}

pub(super) fn staging(image: &RootImage) -> QueueStaging {
    invariant(image.view(|root| {
        let queue = invariant(root.queue());
        QueueStaging {
            expected_epoch: invariant(invariant(queue.expected_epoch()).value()),
            id: invariant(invariant(queue.id()).value()),
            expected_cursor: invariant(invariant(queue.expected_cursor()).value()),
            new_cursor: invariant(invariant(queue.new_cursor()).value()),
            cfg_sq_depth: narrow_u32(invariant(invariant(queue.cfg_sq_depth()).value())),
            cfg_cq_depth: narrow_u32(invariant(invariant(queue.cfg_cq_depth()).value())),
            cfg_software_owner: invariant(invariant(queue.cfg_software_owner()).value()),
            sq_base: invariant(invariant(queue.sq_base()).value()),
            sq_depth: narrow_u32(invariant(invariant(queue.sq_depth()).value())),
            cq_base: invariant(invariant(queue.cq_base()).value()),
            cq_depth: narrow_u32(invariant(invariant(queue.cq_depth()).value())),
            eq_base: invariant(invariant(queue.eq_base()).value()),
            eq_depth: narrow_u32(invariant(invariant(queue.eq_depth()).value())),
        }
    }))
}

pub(super) fn complete(
    image: &RootImage,
    result: MmioResultDetails,
    update: ResponseUpdate<QueueResponseBank>,
) {
    invariant(image.view_mut(|mut root| {
        let response = apply_response_update(read_response(&root), update);
        invariant(invariant(root.q_result_mut()).set_value(u64::from(result.result.code())));
        let [detail0, detail1, detail2] = result.details;
        invariant(invariant(root.q_detail0_mut()).set_value(detail0));
        invariant(invariant(root.q_detail1_mut()).set_value(detail1));
        invariant(invariant(root.q_detail2_mut()).set_value(detail2));
        write_response(&mut root, response);
    }));
}

pub(super) fn complete_bootstrap_batch(
    image: &RootImage,
    staging: BootstrapBatchStaging,
    response: QueueResponseBank,
) {
    invariant(image.view_mut(|mut root| {
        set(
            &mut root,
            QueueRequestWrite::ExpectedEpoch(staging.expected_epoch),
        );
        set(&mut root, QueueRequestWrite::Id(staging.id));
        set(&mut root, QueueRequestWrite::ExpectedCursor(0));
        set(&mut root, QueueRequestWrite::NewCursor(0));
        set(
            &mut root,
            QueueRequestWrite::CfgSqDepth(u64::from(staging.sq_depth)),
        );
        set(
            &mut root,
            QueueRequestWrite::CfgCqDepth(u64::from(staging.cq_depth)),
        );
        set(
            &mut root,
            QueueRequestWrite::CfgSoftwareOwner(staging.cfg_software_owner),
        );
        set(&mut root, QueueRequestWrite::SqBase(staging.sq_base));
        set(
            &mut root,
            QueueRequestWrite::SqDepth(u64::from(staging.sq_depth)),
        );
        set(&mut root, QueueRequestWrite::CqBase(staging.cq_base));
        set(
            &mut root,
            QueueRequestWrite::CqDepth(u64::from(staging.cq_depth)),
        );
        set(&mut root, QueueRequestWrite::EqBase(staging.eq_base));
        set(
            &mut root,
            QueueRequestWrite::EqDepth(u64::from(staging.eq_depth)),
        );
        complete_batch(&mut root, response);
    }));
}

pub(super) fn complete_submission_batch(
    image: &RootImage,
    staging: SubmissionBatchStaging,
    response: QueueResponseBank,
) {
    invariant(image.view_mut(|mut root| {
        set(
            &mut root,
            QueueRequestWrite::ExpectedEpoch(staging.expected_epoch),
        );
        set(&mut root, QueueRequestWrite::Id(staging.id));
        set(
            &mut root,
            QueueRequestWrite::ExpectedCursor(staging.expected_cursor),
        );
        set(&mut root, QueueRequestWrite::NewCursor(staging.new_cursor));
        complete_batch(&mut root, response);
    }));
}

#[derive(Clone, Copy)]
enum QueueRequestWrite {
    ExpectedEpoch(u64),
    Id(u64),
    ExpectedCursor(u64),
    NewCursor(u64),
    CfgSqDepth(u64),
    CfgCqDepth(u64),
    CfgSoftwareOwner(u64),
    SqBase(u64),
    SqDepth(u64),
    CqBase(u64),
    CqDepth(u64),
    EqBase(u64),
    EqDepth(u64),
}

fn set(
    root: &mut pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
    write: QueueRequestWrite,
) {
    match write {
        QueueRequestWrite::ExpectedEpoch(value) => {
            invariant(invariant(root.q_expected_epoch_mut()).set_value(value));
        }
        QueueRequestWrite::Id(value) => invariant(invariant(root.q_id_mut()).set_value(value)),
        QueueRequestWrite::ExpectedCursor(value) => {
            invariant(invariant(root.q_expected_cursor_mut()).set_value(value));
        }
        QueueRequestWrite::NewCursor(value) => {
            invariant(invariant(root.q_new_cursor_mut()).set_value(value));
        }
        QueueRequestWrite::CfgSqDepth(value) => {
            invariant(invariant(root.q_cfg_sq_depth_mut()).set_value(value));
        }
        QueueRequestWrite::CfgCqDepth(value) => {
            invariant(invariant(root.q_cfg_cq_depth_mut()).set_value(value));
        }
        QueueRequestWrite::CfgSoftwareOwner(value) => {
            invariant(invariant(root.q_cfg_software_owner_mut()).set_value(value));
        }
        QueueRequestWrite::SqBase(value) => {
            invariant(invariant(root.q_sq_base_mut()).set_value(value));
        }
        QueueRequestWrite::SqDepth(value) => {
            invariant(invariant(root.q_sq_depth_mut()).set_value(value));
        }
        QueueRequestWrite::CqBase(value) => {
            invariant(invariant(root.q_cq_base_mut()).set_value(value));
        }
        QueueRequestWrite::CqDepth(value) => {
            invariant(invariant(root.q_cq_depth_mut()).set_value(value));
        }
        QueueRequestWrite::EqBase(value) => {
            invariant(invariant(root.q_eq_base_mut()).set_value(value));
        }
        QueueRequestWrite::EqDepth(value) => {
            invariant(invariant(root.q_eq_depth_mut()).set_value(value));
        }
    }
}

fn complete_batch(
    root: &mut pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
    response: QueueResponseBank,
) {
    invariant(invariant(root.q_result_mut()).set_value(u64::from(MmioResult::Ok.code())));
    invariant(invariant(root.q_detail0_mut()).set_value(0));
    invariant(invariant(root.q_detail1_mut()).set_value(0));
    invariant(invariant(root.q_detail2_mut()).set_value(0));
    write_response(root, response);
}

fn read_response(
    root: &pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
) -> QueueResponseBank {
    QueueResponseBank {
        epoch: invariant(invariant(root.q_rsp_epoch()).value()).into(),
        queue_id: invariant(invariant(root.q_rsp_id()).value()).into(),
        sq_producer: invariant(invariant(root.q_rsp_sq_producer()).value()).into(),
        sq_captured: invariant(invariant(root.q_rsp_sq_captured()).value()).into(),
        sq_consumer: invariant(invariant(root.q_rsp_sq_consumer()).value()).into(),
        cq_producer: invariant(invariant(root.q_rsp_cq_producer()).value()).into(),
        cq_consumer: invariant(invariant(root.q_rsp_cq_consumer()).value()).into(),
        eq_producer: invariant(invariant(root.q_rsp_eq_producer()).value()).into(),
        eq_consumer: invariant(invariant(root.q_rsp_eq_consumer()).value()).into(),
    }
}

fn write_response(
    root: &mut pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'_>,
    response: QueueResponseBank,
) {
    invariant(invariant(root.q_rsp_epoch_mut()).set_value(response.epoch.raw()));
    invariant(invariant(root.q_rsp_id_mut()).set_value(response.queue_id.raw()));
    invariant(invariant(root.q_rsp_sq_producer_mut()).set_value(response.sq_producer.raw()));
    invariant(invariant(root.q_rsp_sq_captured_mut()).set_value(response.sq_captured.raw()));
    invariant(invariant(root.q_rsp_sq_consumer_mut()).set_value(response.sq_consumer.raw()));
    invariant(invariant(root.q_rsp_cq_producer_mut()).set_value(response.cq_producer.raw()));
    invariant(invariant(root.q_rsp_cq_consumer_mut()).set_value(response.cq_consumer.raw()));
    invariant(invariant(root.q_rsp_eq_producer_mut()).set_value(response.eq_producer.raw()));
    invariant(invariant(root.q_rsp_eq_consumer_mut()).set_value(response.eq_consumer.raw()));
}
