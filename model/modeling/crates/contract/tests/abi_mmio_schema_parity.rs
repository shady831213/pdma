//! Literal parity coverage for the generated PDMA MMIO schema map.

#[path = "abi_mmio_schema_parity/expected.rs"]
pub mod expected;

use std::{fmt::Debug, hash::Hash};

use hardware_interface_schema::{
    register::{Access, ResetKind},
    register_map::RegisterLocation,
};
use pdma_model_contract::abi::mmio::{
    MMIO_REGION_SIZE, MMIO_REGISTERS, MmioAccess, MmioReset, PdmaMmioMap, PdmaMmioMapRegisterId,
    RegisterId,
};

#[test]
fn public_registry_preserves_all_literal_task_9_rows() {
    // Given: the Task-9-independent, literal ABI table captured from the public v0.9 registry.
    let expected: Vec<_> = expected::rows().collect();

    // When: the existing public registry is observed without a generic map adapter.
    let actual: Vec<_> = MMIO_REGISTERS.iter().copied().collect();

    // Then: the 4 KiB public registry preserves every ID and specification field verbatim.
    assert_eq!(MMIO_REGION_SIZE, 0x1000);
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual.id(), expected.id);
        assert_eq!(actual.name(), expected.name);
        assert_eq!(actual.offset(), expected.offset);
        assert_eq!(actual.width(), expected.width);
        assert_eq!(actual.access(), expected.access);
        assert_eq!(actual.reset(), expected.reset);
        assert_eq!(actual.reset_value(), expected.reset_value);
        assert_eq!(actual.writable_mask(), expected.writable_mask);
    }
}

#[test]
fn generated_pdma_map_preserves_literal_rows_and_exact_starts() {
    // Given: all literal current-contract rows and the generated Task-9 map marker surface.
    let expected: Vec<_> = expected::rows().collect();
    assert_generated_id_derives::<PdmaMmioMapRegisterId>();

    // When: each literal row is queried through the generated typed map and exact-start locate.
    assert_eq!(PdmaMmioMap::SPEC.size(), 0x1000);
    for expected in expected {
        let id = generic_id(expected.id);
        let actual = PdmaMmioMap::register(id);
        assert_eq!(actual.name(), expected.name);
        assert_eq!(actual.offset(), usize::try_from(expected.offset).unwrap());
        assert_eq!(actual.width(), usize::try_from(expected.width).unwrap());
        assert_eq!(actual.access(), generic_access(expected.access));
        assert_eq!(actual.reset_kind(), generic_reset(expected.reset));
        assert_eq!(actual.reset_value(), expected.reset_value);
        assert_eq!(actual.writable_mask(), expected.writable_mask);

        match PdmaMmioMap::locate(usize::try_from(expected.offset).unwrap()) {
            Some(RegisterLocation::Fixed { register, offset }) => {
                assert_eq!(offset, usize::try_from(expected.offset).unwrap());
                assert_eq!(register.id(), id);
                assert_eq!(register, actual);
            }
            Some(RegisterLocation::Repeated { .. }) | None => {
                panic!("PDMA's fixed map must locate every literal exact start")
            }
        }
    }

    // Then: only exact starts locate; representative interior, reserved, and end offsets do not.
    for offset in [0x001, 0x00c, 0x011, 0x16d, 0x271, 0xfff, 0x1000] {
        assert_eq!(PdmaMmioMap::locate(offset), None);
    }
}

const fn assert_generated_id_derives<T>()
where
    T: Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd,
{
}

const fn generic_access(access: MmioAccess) -> Access {
    match access {
        MmioAccess::ReadOnly => Access::ReadOnly,
        MmioAccess::ReadWrite => Access::ReadWrite,
        MmioAccess::WriteOnly => Access::WriteOnly,
    }
}

const fn generic_reset(reset: MmioReset) -> ResetKind {
    match reset {
        MmioReset::Constant => ResetKind::Constant,
        MmioReset::Zero => ResetKind::Zero,
        MmioReset::PostReset => ResetKind::PostReset,
    }
}

const fn generic_id(id: RegisterId) -> PdmaMmioMapRegisterId {
    match id {
        RegisterId::GMagic => PdmaMmioMapRegisterId::GMagic,
        RegisterId::GMmioAbiVersion => PdmaMmioMapRegisterId::GMmioAbiVersion,
        RegisterId::GReqAbiVersion => PdmaMmioMapRegisterId::GReqAbiVersion,
        RegisterId::GReqFeatures => PdmaMmioMapRegisterId::GReqFeatures,
        RegisterId::GResetExpectedEpoch => PdmaMmioMapRegisterId::GResetExpectedEpoch,
        RegisterId::GCommand => PdmaMmioMapRegisterId::GCommand,
        RegisterId::GResult => PdmaMmioMapRegisterId::GResult,
        RegisterId::GDetail0 => PdmaMmioMapRegisterId::GDetail0,
        RegisterId::GDetail1 => PdmaMmioMapRegisterId::GDetail1,
        RegisterId::GDetail2 => PdmaMmioMapRegisterId::GDetail2,
        RegisterId::GRspAbiVersion => PdmaMmioMapRegisterId::GRspAbiVersion,
        RegisterId::GRspRunState => PdmaMmioMapRegisterId::GRspRunState,
        RegisterId::GRspFeatures => PdmaMmioMapRegisterId::GRspFeatures,
        RegisterId::GRspEpoch => PdmaMmioMapRegisterId::GRspEpoch,
        RegisterId::GMaxRingDepth => PdmaMmioMapRegisterId::GMaxRingDepth,
        RegisterId::GMaxIndirectDestinations => PdmaMmioMapRegisterId::GMaxIndirectDestinations,
        RegisterId::GMaxTransferBytes => PdmaMmioMapRegisterId::GMaxTransferBytes,
        RegisterId::GMaxStridedRows => PdmaMmioMapRegisterId::GMaxStridedRows,
        RegisterId::GMaxStride => PdmaMmioMapRegisterId::GMaxStride,
        RegisterId::QExpectedEpoch => PdmaMmioMapRegisterId::QExpectedEpoch,
        RegisterId::QId => PdmaMmioMapRegisterId::QId,
        RegisterId::QExpectedCursor => PdmaMmioMapRegisterId::QExpectedCursor,
        RegisterId::QNewCursor => PdmaMmioMapRegisterId::QNewCursor,
        RegisterId::QCfgSqDepth => PdmaMmioMapRegisterId::QCfgSqDepth,
        RegisterId::QCfgCqDepth => PdmaMmioMapRegisterId::QCfgCqDepth,
        RegisterId::QCfgEventPolicy => PdmaMmioMapRegisterId::QCfgEventPolicy,
        RegisterId::QCfgSoftwareOwner => PdmaMmioMapRegisterId::QCfgSoftwareOwner,
        RegisterId::QSqBase => PdmaMmioMapRegisterId::QSqBase,
        RegisterId::QSqDepth => PdmaMmioMapRegisterId::QSqDepth,
        RegisterId::QCqBase => PdmaMmioMapRegisterId::QCqBase,
        RegisterId::QCqDepth => PdmaMmioMapRegisterId::QCqDepth,
        RegisterId::QEqBase => PdmaMmioMapRegisterId::QEqBase,
        RegisterId::QEqDepth => PdmaMmioMapRegisterId::QEqDepth,
        RegisterId::QCommand => PdmaMmioMapRegisterId::QCommand,
        RegisterId::QResult => PdmaMmioMapRegisterId::QResult,
        RegisterId::QDetail0 => PdmaMmioMapRegisterId::QDetail0,
        RegisterId::QDetail1 => PdmaMmioMapRegisterId::QDetail1,
        RegisterId::QDetail2 => PdmaMmioMapRegisterId::QDetail2,
        RegisterId::QRspEpoch => PdmaMmioMapRegisterId::QRspEpoch,
        RegisterId::QRspId => PdmaMmioMapRegisterId::QRspId,
        RegisterId::QRspSqProducer => PdmaMmioMapRegisterId::QRspSqProducer,
        RegisterId::QRspSqCaptured => PdmaMmioMapRegisterId::QRspSqCaptured,
        RegisterId::QRspSqConsumer => PdmaMmioMapRegisterId::QRspSqConsumer,
        RegisterId::QRspCqProducer => PdmaMmioMapRegisterId::QRspCqProducer,
        RegisterId::QRspCqConsumer => PdmaMmioMapRegisterId::QRspCqConsumer,
        RegisterId::QRspEqProducer => PdmaMmioMapRegisterId::QRspEqProducer,
        RegisterId::QRspEqConsumer => PdmaMmioMapRegisterId::QRspEqConsumer,
        RegisterId::IExpectedEpoch => PdmaMmioMapRegisterId::IExpectedEpoch,
        RegisterId::ISourceId => PdmaMmioMapRegisterId::ISourceId,
        RegisterId::ISourceKind => PdmaMmioMapRegisterId::ISourceKind,
        RegisterId::IRouteKind => PdmaMmioMapRegisterId::IRouteKind,
        RegisterId::IRouteVector => PdmaMmioMapRegisterId::IRouteVector,
        RegisterId::ICommand => PdmaMmioMapRegisterId::ICommand,
        RegisterId::IResult => PdmaMmioMapRegisterId::IResult,
        RegisterId::IDetail0 => PdmaMmioMapRegisterId::IDetail0,
        RegisterId::IDetail1 => PdmaMmioMapRegisterId::IDetail1,
        RegisterId::IDetail2 => PdmaMmioMapRegisterId::IDetail2,
        RegisterId::IRspEpoch => PdmaMmioMapRegisterId::IRspEpoch,
        RegisterId::IRspSourceId => PdmaMmioMapRegisterId::IRspSourceId,
        RegisterId::IRspSourceKind => PdmaMmioMapRegisterId::IRspSourceKind,
        RegisterId::IRspState => PdmaMmioMapRegisterId::IRspState,
        RegisterId::IRspRouteKind => PdmaMmioMapRegisterId::IRspRouteKind,
        RegisterId::IRspNotificationKind => PdmaMmioMapRegisterId::IRspNotificationKind,
        RegisterId::IRspVector => PdmaMmioMapRegisterId::IRspVector,
        RegisterId::IRspMessageId => PdmaMmioMapRegisterId::IRspMessageId,
        RegisterId::IRspPendingMessages => PdmaMmioMapRegisterId::IRspPendingMessages,
    }
}
