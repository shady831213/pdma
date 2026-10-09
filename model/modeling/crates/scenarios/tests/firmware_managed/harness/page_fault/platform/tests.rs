use super::*;
use crate::harness::page_fault::{FaultAccess, FaultReport};
use pdma_model_contract::memory::ContextAttributes;

#[test]
fn fault_catalog_rejects_foreign_unknown_and_retired_sources() {
    // Given: one live grant in a two-channel platform instance.
    let sources = Sources::new(2);
    let channel = ChannelId::new(1);
    let function = FunctionId::new(3);
    let mut handler = sources.handler(function, &[channel]).unwrap();
    let report = FaultReport {
        identity: MemoryIdentity {
            function,
            attributes: ContextAttributes::DEFAULT,
        },
        tag: sources.tag(channel).unwrap(),
        address: 0x1000.into(),
        access: FaultAccess::Read,
    };
    // When: report identity or source falls outside the grant, or its handler retires.
    let wrong_function = FaultReport {
        identity: MemoryIdentity {
            function: FunctionId::new(4),
            ..report.identity
        },
        ..report.clone()
    };
    let ungranted = FaultReport {
        tag: sources.tag(ChannelId::new(0)).unwrap(),
        ..report
    };
    let foreign = FaultReport {
        tag: Sources::new(2).tag(channel).unwrap(),
        ..report
    };
    // Then: only the current grant's source and Function resolve, with no partial invalid catalog.
    assert_eq!(handler.resolve(&report), Some(channel));
    assert_eq!(handler.resolve(&wrong_function), None);
    assert_eq!(handler.resolve(&ungranted), None);
    assert_eq!(handler.resolve(&foreign), None);
    assert!(
        sources
            .handler(function, &[channel, ChannelId::new(2)])
            .is_none()
    );
    handler.retire();
    assert_eq!(handler.resolve(&report), None);
}
