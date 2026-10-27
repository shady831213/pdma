#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use hardware_interface_io::memory::{MemoryAddress, MemoryAlignment, MemoryAlignmentError};
use hardware_interface_io::memory::{MemoryRegion, MemoryRegionError};
use support::{alignment, region};

#[test]
fn memory_alignment_rejects_zero_and_non_power_of_two() {
    // Given alignment values that cannot express an address requirement.
    // When each value crosses the checked constructor boundary.
    let zero = MemoryAlignment::try_new(0);
    let odd = MemoryAlignment::try_new(3);

    // Then literal typed failures retain the rejected values.
    assert_eq!(zero, Err(MemoryAlignmentError::Invalid { alignment: 0 }));
    assert_eq!(odd, Err(MemoryAlignmentError::Invalid { alignment: 3 }));
}

#[test]
fn memory_region_rejects_zero_size_before_other_checks() {
    // Given an empty region whose base would also violate its alignment.
    // When the region is constructed.
    let result = MemoryRegion::try_new(MemoryAddress::new(3), 0, alignment(8));

    // Then empty-region precedence is literal.
    assert_eq!(result, Err(MemoryRegionError::Empty));
}

#[test]
fn memory_region_rejects_misaligned_base_before_checked_end() {
    // Given a misaligned region whose exclusive end would also overflow.
    // When the region is constructed.
    let result = MemoryRegion::try_new(MemoryAddress::new(u64::MAX), 8, alignment(8));

    // Then base alignment wins before checked-end arithmetic.
    assert_eq!(
        result,
        Err(MemoryRegionError::MisalignedBase {
            base: MemoryAddress::new(u64::MAX),
            alignment: alignment(8),
        })
    );
}

#[test]
fn memory_region_rejects_checked_end_before_storage_size() {
    // Given an aligned region whose end overflows and length exhausts narrow storage.
    // When the region is constructed.
    let result = MemoryRegion::try_new(MemoryAddress::new(u64::MAX - 7), u64::MAX, alignment(8));

    // Then checked address exhaustion is reported first.
    assert_eq!(
        result,
        Err(MemoryRegionError::AddressOverflow {
            base: MemoryAddress::new(u64::MAX - 7),
            length: u64::MAX,
        })
    );
}

#[test]
fn memory_region_reports_storage_conversion_on_narrow_targets() {
    // Given the largest checked exclusive extent starting at zero.
    // When the host storage width is narrower than the address width.
    let result = MemoryRegion::try_new(MemoryAddress::new(0), u64::MAX, alignment(1));

    // Then conversion exhaustion is typed; 64-bit hosts retain checked metadata.
    if usize::BITS < u64::BITS {
        assert_eq!(
            result,
            Err(MemoryRegionError::StorageSizeOutOfRange { length: u64::MAX })
        );
    } else {
        assert_eq!(result, Ok(region(0, u64::MAX, 1)));
    }
}
