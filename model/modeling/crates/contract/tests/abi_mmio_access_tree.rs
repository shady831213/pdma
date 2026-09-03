//! Additive grouped-access coverage for the PDMA MMIO map.

use hardware_interface_schema::{Encode, RegisterLayout, SchemaCodecError};
use pdma_model_contract::abi::mmio::{
    GlobalResponseRegistersView, GlobalResponseRegistersViewMut, IrqResponseRegistersView,
    IrqResponseRegistersViewMut, PdmaMmioMap, PdmaMmioMapRegisterId, PdmaMmioMapView,
    PdmaMmioMapViewMut, QueueResponseRegistersView, QueueResponseRegistersViewMut, RegisterId,
    global, irq, queue,
};

macro_rules! legacy_root_entries {
    ($view:ident, $view_mut:ident; $($id:ident => $accessor:ident, $mutable:ident),+ $(,)?) => {{
        $(let _ = $view.$accessor()?; let _ = $view_mut.$mutable()?;)+
        [$(PdmaMmioMapRegisterId::$id),+]
    }};
}

type PdmaMmioMapValues = <PdmaMmioMap as RegisterLayout>::Value;
type PdmaMmioMapRaw = <PdmaMmioMapValues as Encode>::Encoded;

const fn type_exists<T>() {}

macro_rules! assert_grouped_values {
    ($group:ident, $compact:ident; $($group_leaf:ident => $compact_leaf:ident),+ $(,)?) => {
        $(assert_eq!($group.$group_leaf()?.value()?, $compact.$compact_leaf()?.value()?);)+
    };
}

#[test]
fn legacy_compact_response_raw_values_and_views_remain_public()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the legacy compact raw owners for every response family.
    let global_raw = global::GlobalResponseRegistersRaw::ZERO;
    let queue_raw = queue::QueueResponseRegistersRaw::ZERO;
    let irq_raw = irq::IrqResponseRegistersRaw::ZERO;

    // When: each owner is decoded into values and borrowed through its legacy view.
    let global_values = global::GlobalResponseRegistersValues::try_from(global_raw)?;
    let queue_values = queue::QueueResponseRegistersValues::try_from(queue_raw)?;
    let irq_values = irq::IrqResponseRegistersValues::try_from(irq_raw)?;
    let global_view = GlobalResponseRegistersView::new(global_raw.as_bytes())?;
    let queue_view = QueueResponseRegistersView::new(queue_raw.as_bytes())?;
    let irq_view = IrqResponseRegistersView::new(irq_raw.as_bytes())?;

    // Then: every pre-existing raw, values, and view identity remains usable.
    assert_eq!(global_values, global::GlobalResponseRegistersValues::ZERO);
    assert_eq!(queue_values, queue::QueueResponseRegistersValues::ZERO);
    assert_eq!(irq_values, irq::IrqResponseRegistersValues::ZERO);
    assert_eq!(global_view.abi_version()?.value()?, 0);
    assert_eq!(queue_view.epoch()?.value()?, 0);
    assert_eq!(irq_view.epoch()?.value()?, 0);
    Ok(())
}

#[test]
fn projection_matches_compact_response_bytes_through_grouped_roots()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: literal compact response raw bytes for the three response families.
    let global_raw = global::GlobalResponseRegistersRaw::new([
        0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02,
        0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff, 0x00, 0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90, 0x67, 0x45, 0x23, 0x01,
    ]);
    let queue_raw = queue::QueueResponseRegistersRaw::new([
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12,
        0x11, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33,
        0x32, 0x31, 0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0x58, 0x57, 0x56, 0x55, 0x54,
        0x53, 0x52, 0x51, 0x68, 0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61, 0x78, 0x77, 0x76, 0x75,
        0x74, 0x73, 0x72, 0x71, 0x88, 0x87, 0x86, 0x85, 0x84, 0x83, 0x82, 0x81,
    ]);
    let irq_raw = irq::IrqResponseRegistersRaw::new([
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12,
        0x11, 0x24, 0x23, 0x22, 0x21, 0x3b, 0, 0, 0, 0x34, 0x33, 0x32, 0x31, 2, 0, 0, 0, 0x48,
        0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0xa8, 0xa7, 0xa6, 0xa5, 0xa4, 0xa3, 0xa2, 0xa1,
        0xb8, 0xb7, 0xb6, 0xb5, 0xb4, 0xb3, 0xb2, 0xb1,
    ]);
    let global_compact = GlobalResponseRegistersView::new(global_raw.as_bytes())?;
    let queue_compact = QueueResponseRegistersView::new(queue_raw.as_bytes())?;
    let irq_compact = IrqResponseRegistersView::new(irq_raw.as_bytes())?;
    let mut root_bytes = [0_u8; 0x1000];
    root_bytes[0x040..0x06c].copy_from_slice(global_raw.as_bytes());
    root_bytes[0x188..0x1d0].copy_from_slice(queue_raw.as_bytes());
    root_bytes[0x240..0x278].copy_from_slice(irq_raw.as_bytes());

    // When: the response ranges are projected through the grouped root views.
    let root = PdmaMmioMapView::new(&root_bytes)?;
    let global = root.global()?;
    let queue = root.queue()?;
    let irq = root.irq()?;

    // Then: every contiguous compact byte range and every grouped response leaf agree.
    assert_eq!(&root_bytes[0x040..0x06c], global_raw.as_bytes());
    assert_eq!(&root_bytes[0x188..0x1d0], queue_raw.as_bytes());
    assert_eq!(&root_bytes[0x240..0x278], irq_raw.as_bytes());
    assert_grouped_values!(global, global_compact;
        rsp_abi_version => abi_version, rsp_run_state => run_state, rsp_features => features,
        rsp_epoch => epoch, max_ring_depth => max_ring_depth,
        max_indirect_destinations => max_indirect_destinations,
        max_transfer_bytes => max_transfer_bytes, max_strided_rows => max_strided_rows,
        max_stride => max_stride,
    );
    assert_grouped_values!(queue, queue_compact;
        rsp_epoch => epoch, rsp_id => queue_id, rsp_sq_producer => sq_producer,
        rsp_sq_captured => sq_captured, rsp_sq_consumer => sq_consumer,
        rsp_cq_producer => cq_producer, rsp_cq_consumer => cq_consumer,
        rsp_eq_producer => eq_producer, rsp_eq_consumer => eq_consumer,
    );
    assert_grouped_values!(irq, irq_compact;
        rsp_epoch => epoch, rsp_source_id => source_id, rsp_source_kind => source_kind,
        rsp_state => state, rsp_route_kind => route_kind,
        rsp_notification_kind => notification_kind, rsp_vector => vector,
        rsp_message_id => message_id, rsp_pending_messages => pending_messages,
    );
    Ok(())
}

#[test]
fn grouped_and_flat_mutations_alias_and_reject_invalid_images()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a root image and malformed images with distinct before-bytes.
    let mut bytes = [0_u8; 0x1000];
    let mut short = [0xa5_u8; 0xfff];
    let short_before = short;
    let before = bytes;

    // When: writable leaves are selected through groups and flats, then a malformed root is tried.
    PdmaMmioMapViewMut::new(&mut bytes)?
        .global_mut()?
        .req_abi_version_mut()?
        .set_value(0x1122_3344)?;
    PdmaMmioMapViewMut::new(&mut bytes)?
        .g_req_abi_version_mut()?
        .set_value(0x5566_7788)?;
    PdmaMmioMapViewMut::new(&mut bytes)?
        .queue_mut()?
        .expected_epoch_mut()?
        .set_value(0x0102_0304_0506_0708)?;
    PdmaMmioMapViewMut::new(&mut bytes)?
        .q_expected_epoch_mut()?
        .set_value(0x1112_1314_1516_1718)?;
    PdmaMmioMapViewMut::new(&mut bytes)?
        .irq_mut()?
        .route_vector_mut()?
        .set_value(0x2122_2324_2526_2728)?;
    PdmaMmioMapViewMut::new(&mut bytes)?
        .i_route_vector_mut()?
        .set_value(0x3132_3334_3536_3738)?;
    let short_error = PdmaMmioMapViewMut::new(&mut short).err();

    // Then: groups and flats share bytes while the exact malformed error preserves its image.
    assert_eq!(&bytes[0x008..0x00c], &[0x88, 0x77, 0x66, 0x55]);
    assert_eq!(
        &bytes[0x100..0x108],
        &[0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11]
    );
    assert_eq!(
        &bytes[0x218..0x220],
        &[0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31]
    );
    assert_eq!(
        short_error,
        Some(SchemaCodecError::BufferTooShort {
            required: 0x1000,
            actual: 0xfff
        })
    );
    assert_eq!(short, short_before);
    assert_ne!(bytes, before);
    Ok(())
}

#[test]
fn legacy_generated_root_symbols_and_ids_remain_public() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a full root image and every generated legacy root accessor/identity.
    let bytes = [0_u8; 0x1000];
    let mut mutable_bytes = [0xa5_u8; 0x1000];
    let root = PdmaMmioMapView::new(&bytes)?;
    let mut root_mut = PdmaMmioMapViewMut::new(&mut mutable_bytes)?;
    let expected = legacy_root_entries!(root, root_mut;
        GMagic => g_magic, g_magic_mut, GMmioAbiVersion => g_mmio_abi_version, g_mmio_abi_version_mut,
        GReqAbiVersion => g_req_abi_version, g_req_abi_version_mut, GReqFeatures => g_req_features, g_req_features_mut,
        GResetExpectedEpoch => g_reset_expected_epoch, g_reset_expected_epoch_mut, GCommand => g_command, g_command_mut,
        GResult => g_result, g_result_mut, GDetail0 => g_detail0, g_detail0_mut, GDetail1 => g_detail1, g_detail1_mut,
        GDetail2 => g_detail2, g_detail2_mut, GRspAbiVersion => g_rsp_abi_version, g_rsp_abi_version_mut,
        GRspRunState => g_rsp_run_state, g_rsp_run_state_mut, GRspFeatures => g_rsp_features, g_rsp_features_mut,
        GRspEpoch => g_rsp_epoch, g_rsp_epoch_mut, GMaxRingDepth => g_max_ring_depth, g_max_ring_depth_mut,
        GMaxIndirectDestinations => g_max_indirect_destinations, g_max_indirect_destinations_mut,
        GMaxTransferBytes => g_max_transfer_bytes, g_max_transfer_bytes_mut,
        GMaxStridedRows => g_max_strided_rows, g_max_strided_rows_mut, GMaxStride => g_max_stride, g_max_stride_mut,
        QExpectedEpoch => q_expected_epoch, q_expected_epoch_mut, QId => q_id, q_id_mut,
        QExpectedCursor => q_expected_cursor, q_expected_cursor_mut, QNewCursor => q_new_cursor, q_new_cursor_mut,
        QCfgSqDepth => q_cfg_sq_depth, q_cfg_sq_depth_mut, QCfgCqDepth => q_cfg_cq_depth, q_cfg_cq_depth_mut,
        QCfgEventPolicy => q_cfg_event_policy, q_cfg_event_policy_mut, QCfgSoftwareOwner => q_cfg_software_owner, q_cfg_software_owner_mut,
        QSqBase => q_sq_base, q_sq_base_mut, QSqDepth => q_sq_depth, q_sq_depth_mut,
        QCqBase => q_cq_base, q_cq_base_mut, QCqDepth => q_cq_depth, q_cq_depth_mut,
        QEqBase => q_eq_base, q_eq_base_mut, QEqDepth => q_eq_depth, q_eq_depth_mut,
        QCommand => q_command, q_command_mut, QResult => q_result, q_result_mut,
        QDetail0 => q_detail0, q_detail0_mut, QDetail1 => q_detail1, q_detail1_mut, QDetail2 => q_detail2, q_detail2_mut,
        QRspEpoch => q_rsp_epoch, q_rsp_epoch_mut, QRspId => q_rsp_id, q_rsp_id_mut,
        QRspSqProducer => q_rsp_sq_producer, q_rsp_sq_producer_mut, QRspSqCaptured => q_rsp_sq_captured, q_rsp_sq_captured_mut,
        QRspSqConsumer => q_rsp_sq_consumer, q_rsp_sq_consumer_mut, QRspCqProducer => q_rsp_cq_producer, q_rsp_cq_producer_mut,
        QRspCqConsumer => q_rsp_cq_consumer, q_rsp_cq_consumer_mut, QRspEqProducer => q_rsp_eq_producer, q_rsp_eq_producer_mut,
        QRspEqConsumer => q_rsp_eq_consumer, q_rsp_eq_consumer_mut, IExpectedEpoch => i_expected_epoch, i_expected_epoch_mut,
        ISourceId => i_source_id, i_source_id_mut, ISourceKind => i_source_kind, i_source_kind_mut,
        IRouteKind => i_route_kind, i_route_kind_mut, IRouteVector => i_route_vector, i_route_vector_mut,
        ICommand => i_command, i_command_mut, IResult => i_result, i_result_mut,
        IDetail0 => i_detail0, i_detail0_mut, IDetail1 => i_detail1, i_detail1_mut, IDetail2 => i_detail2, i_detail2_mut,
        IRspEpoch => i_rsp_epoch, i_rsp_epoch_mut, IRspSourceId => i_rsp_source_id, i_rsp_source_id_mut,
        IRspSourceKind => i_rsp_source_kind, i_rsp_source_kind_mut, IRspState => i_rsp_state, i_rsp_state_mut,
        IRspRouteKind => i_rsp_route_kind, i_rsp_route_kind_mut,
        IRspNotificationKind => i_rsp_notification_kind, i_rsp_notification_kind_mut,
        IRspVector => i_rsp_vector, i_rsp_vector_mut, IRspMessageId => i_rsp_message_id, i_rsp_message_id_mut,
        IRspPendingMessages => i_rsp_pending_messages, i_rsp_pending_messages_mut,
    );

    // When: all flat symbols and their stable IDs are checked against generated metadata.
    let first: RegisterId = PdmaMmioMapRegisterId::GMagic;
    type_exists::<PdmaMmioMap>();
    type_exists::<PdmaMmioMapRegisterId>();
    type_exists::<PdmaMmioMapRaw>();
    type_exists::<PdmaMmioMapValues>();
    type_exists::<PdmaMmioMapView<'static>>();
    type_exists::<PdmaMmioMapViewMut<'static>>();
    type_exists::<global::GlobalResponseRegistersRaw>();
    type_exists::<global::GlobalResponseRegistersValues>();
    type_exists::<GlobalResponseRegistersView<'static>>();
    type_exists::<GlobalResponseRegistersViewMut<'static>>();
    type_exists::<queue::QueueResponseRegistersRaw>();
    type_exists::<queue::QueueResponseRegistersValues>();
    type_exists::<QueueResponseRegistersView<'static>>();
    type_exists::<QueueResponseRegistersViewMut<'static>>();
    type_exists::<irq::IrqResponseRegistersRaw>();
    type_exists::<irq::IrqResponseRegistersValues>();
    type_exists::<IrqResponseRegistersView<'static>>();
    type_exists::<IrqResponseRegistersViewMut<'static>>();

    // Then: every legacy root accessor and generated ID keeps its identity/order.
    assert_eq!(first, expected[0]);
    assert_eq!(PdmaMmioMapRegisterId::ALL, expected);
    assert_eq!(PdmaMmioMap::ACCESS_PATHS.len(), expected.len());
    for (path, id) in PdmaMmioMap::ACCESS_PATHS.iter().zip(expected) {
        assert_eq!(path.leaf().target(), id);
        assert_eq!(path.groups().len(), 1);
        assert!(
            path.groups()
                .first()
                .is_some_and(|group| matches!(group.name(), "global" | "queue" | "irq"))
        );
    }
    Ok(())
}
