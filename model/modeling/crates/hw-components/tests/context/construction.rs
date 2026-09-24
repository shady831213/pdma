//! Context-table construction reports its own capacity and layout failures.
use hardware_interface_schema::RegisterBankError;
use pdma_model_contract::hw_components::context::ContextManagementRegisters;
use pdma_model_hw_components::context::{ContextConfigError, ContextTable};

#[test]
fn empty_context_or_bitmap_bank_retains_the_layout_error() {
    // Given: zero context capacity or zero channels for the failure bitmap.
    for (contexts, channels) in [(0, 1), (1, 0)] {
        // When: the table is constructed.
        let error = ContextTable::new(contexts, channels, 0).unwrap_err();
        // Then: the failure describes context layout, not a window channel count.
        assert!(matches!(
            error,
            ContextConfigError::Layout(RegisterBankError::ZeroCount)
        ));
    }
}

#[test]
fn misaligned_and_overflowing_apertures_keep_distinct_layout_failures() {
    // Given: one misaligned base and one aligned base with insufficient address space.
    for (base, expected) in [
        (1, RegisterBankError::Alignment),
        (usize::MAX & !7, RegisterBankError::Overflow),
    ] {
        // When: the table is constructed at that base.
        let error = ContextTable::new(1, 1, base).unwrap_err();
        // Then: the original schema cause is retained.
        match error {
            ContextConfigError::Layout(actual) => assert_eq!(actual, expected),
            other @ (ContextConfigError::InvalidCapacity | ContextConfigError::Allocation(_)) => {
                panic!("expected layout failure, got {other:?}");
            }
        }
    }
}

#[test]
fn unrepresentable_context_capacity_is_not_a_window_error() {
    // Given: a count too large for the table's capacity register, where representable by usize.
    if let Ok(count) = usize::try_from(u64::from(u32::MAX) + 1) {
        // When: construction validates capacity before allocating entry storage.
        let error = ContextTable::new(count, 1, 0).unwrap_err();
        // Then: it identifies context capacity, not channel ownership or geometry.
        assert!(matches!(error, ContextConfigError::InvalidCapacity));
    }
}

#[test]
fn valid_table_reports_capacity_at_its_relocated_aperture() {
    // Given: context capacity differs from channel count and uses a nonzero base.
    let mut table = ContextTable::new(3, 65, 0x1000).unwrap();
    let mut bytes = [0; 4];
    // When: firmware reads the capacity register.
    table
        .management_read(
            0x1000 + ContextManagementRegisters::CAPACITY.offset(),
            &mut bytes,
        )
        .unwrap();
    // Then: discovery reports the configured entry count.
    assert_eq!(u32::from_le_bytes(bytes), 3);
}
