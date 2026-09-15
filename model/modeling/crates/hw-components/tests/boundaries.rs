//! Complete-transaction validation and firmware management isolation.

use hardware_interface_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::{WINDOW_CONTROL_BASE, WINDOW_STATUS_BASE, WindowControl};
use pdma_model_hw_components::{AccessError, AccessWindow, WindowConfigError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn constructor_rejects_overlap_with_reserved_addresses() -> TestResult {
    // Given: the first reserved management address.
    let control = usize::try_from(WINDOW_CONTROL_BASE)?;
    // When: downstream extents end before, at, or beyond that address.
    for extent in [0, 4, control] {
        assert!(AccessWindow::new(extent).is_ok());
    }
    let overlap = AccessWindow::new(control + 1);
    let overflow = AccessWindow::new(usize::MAX);
    // Then: only overlapping extents fail with the typed configuration error.
    assert!(matches!(
        overlap,
        Err(WindowConfigError::ReservedAddressOverlap)
    ));
    assert!(matches!(
        overflow,
        Err(WindowConfigError::ReservedAddressOverlap)
    ));
    Ok(())
}

#[test]
fn malformed_management_transactions_preserve_readiness() -> TestResult {
    // Given: firmware can manage an otherwise closed window.
    let mut window = AccessWindow::new(4)?;
    let control = usize::try_from(WINDOW_CONTROL_BASE)?;
    let status = usize::try_from(WINDOW_STATUS_BASE)?;
    for ready in [0_u32, 1] {
        window.management_write(control, &ready.to_le_bytes())?;
        // When: management sends invalid widths, addresses or reserved bits.
        for (address, bytes) in [
            (control, vec![]),
            (control, vec![1]),
            (control, vec![1; 8]),
            (control + 1, vec![1; 4]),
            (control, 3_u32.to_le_bytes().to_vec()),
            (0, vec![1; 4]),
            (status, vec![1; 4]),
            (usize::MAX, vec![1; 4]),
        ] {
            let rejected = window.management_write(address, &bytes);
            // Then: all invalid writes leave the authoritative control bit unchanged.
            assert!(rejected.is_err());
            assert_eq!(window.is_open(), ready == 1);
        }
        let mut output = [0xa5; 4];
        assert_eq!(
            window.management_read(status, &mut output),
            Err(RegisterAccessError::OutsideWindow { address: status })
        );
        assert_eq!(output, [0xa5; 4]);
        window.management_read(control, &mut output)?;
        assert_eq!(output, ready.to_le_bytes());
    }
    Ok(())
}

#[test]
fn complete_downstream_bounds_are_checked_before_forwarding() -> TestResult {
    // Given: real downstream storage extends beyond the permitted aperture.
    let mut window = AccessWindow::new(3)?;
    window.management_write(usize::try_from(WINDOW_CONTROL_BASE)?, &1_u32.to_le_bytes())?;
    let mut registers = RegisterWindow::<WindowControl>::new(0);
    // When: accesses cross the extent, start outside it, or overflow.
    for (address, width) in [(0, 4), (2, 4), (3, 1), (0, 0), (usize::MAX, 4)] {
        let bytes = vec![1; width];
        let mut output = vec![0xa5; width];
        let write = window.write(address, &bytes, |address, bytes| {
            registers.write(address, bytes)
        });
        let read = window.read(address, &mut output, |address, bytes| {
            registers.read(address, bytes)
        });
        // Then: the aperture rejects the complete transaction before downstream access.
        assert_eq!(write, Err(AccessError::Unmapped));
        assert_eq!(read, Err(AccessError::Unmapped));
        assert_eq!(output, vec![0xa5; width]);
        assert!(!registers.values.open.enabled());
        assert!(window.is_open());
    }
    Ok(())
}

#[test]
fn malformed_status_reads_preserve_output_and_ready() -> TestResult {
    // Given: open hardware and downstream storage unrelated to status.
    let mut window = AccessWindow::new(4)?;
    window.management_write(usize::try_from(WINDOW_CONTROL_BASE)?, &1_u32.to_le_bytes())?;
    let status = usize::try_from(WINDOW_STATUS_BASE)?;
    let mut registers = RegisterWindow::<WindowControl>::new(0);
    // When: status reads are too short, span its boundary, or start inside its leaf.
    for (address, width) in [(status, 0), (status, 1), (status, 8), (status + 1, 4)] {
        let mut output = vec![0xa5; width];
        let read = window.read(address, &mut output, |address, bytes| {
            registers.read(address, bytes)
        });
        // Then: generated register checks reject them before output or control changes.
        assert!(matches!(read, Err(AccessError::Register(_))));
        assert_eq!(output, vec![0xa5; width]);
        assert!(window.is_open());
    }
    Ok(())
}

#[test]
fn closing_blocks_further_downstream_transactions() -> TestResult {
    // Given: host has already written downstream while access is open.
    let mut window = AccessWindow::new(4)?;
    let control = usize::try_from(WINDOW_CONTROL_BASE)?;
    let mut registers = RegisterWindow::<WindowControl>::new(0);
    window.management_write(control, &1_u32.to_le_bytes())?;
    window.write(0, &1_u32.to_le_bytes(), |address, bytes| {
        registers.write(address, bytes)
    })?;
    // When: firmware closes access before the next host write.
    window.management_write(control, &0_u32.to_le_bytes())?;
    let result = window.write(0, &0_u32.to_le_bytes(), |address, bytes| {
        registers.write(address, bytes)
    });
    // Then: the next transaction is blocked and the prior hardware value survives.
    assert_eq!(result, Err(AccessError::Closed));
    assert!(registers.values.open.enabled());
    Ok(())
}
