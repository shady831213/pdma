//! Independent Host apertures against generated downstream storage.
use hardware_interface_schema::RegisterWindow;
use pdma_model_contract::{ChannelBlock, ChannelId, DeviceRegisters, WINDOW_CONTROL_BASE};
use pdma_model_hw_components::{AccessError, ChannelWindows};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn opening_one_channel_preserves_closed_peer() -> TestResult {
    // Given: two closed gates and real channel register storage.
    let mut windows = ChannelWindows::new(2)?;
    let first = ChannelId::new(0).register_offset().unwrap();
    let second = ChannelId::new(1).register_offset().unwrap();
    let mut peer = RegisterWindow::<ChannelBlock>::new(second);
    let mut owned = RegisterWindow::<ChannelBlock>::new(first);
    windows.management_write(usize::try_from(WINDOW_CONTROL_BASE)?, &1_u32.to_le_bytes())?;
    // When: Host writes both apertures.
    windows.write(first, &17_u64.to_le_bytes(), |a, b| owned.write(a, b))?;
    let rejected = windows.write(second, &42_u64.to_le_bytes(), |a, b| peer.write(a, b));
    // Then: only the granted aperture changes.
    assert_eq!(owned.values.command.base.raw(), 17);
    assert_eq!(rejected, Err(AccessError::Closed));
    assert_eq!(peer.values.command.base.raw(), 0);
    Ok(())
}

#[test]
fn open_neighbors_do_not_authorize_cross_boundary_transactions() -> TestResult {
    // Given: both adjacent channels are open.
    let mut windows = ChannelWindows::new(2)?;
    for index in 0..2 {
        windows.management_write(
            usize::try_from(WINDOW_CONTROL_BASE)? + index * 16,
            &1_u32.to_le_bytes(),
        )?;
    }
    let boundary = ChannelId::new(1).register_offset().unwrap();
    let mut invoked = false;
    // When: a write straddles adjacent windows.
    let result = windows.write(boundary - 4, &[0; 8], |_, _| {
        invoked = true;
        Ok::<_, ()>(())
    });
    // Then: authorization rejects it before downstream invocation.
    assert_eq!(result, Err(AccessError::Unmapped));
    assert!(!invoked);
    Ok(())
}

#[test]
fn closed_and_outside_access_never_reaches_downstream() -> TestResult {
    // Given: all windows start closed.
    let windows = ChannelWindows::new(2)?;
    // When: Host accesses globals, channels, or nonexistent apertures.
    for (address, expected) in [
        (0, AccessError::Closed),
        (16, AccessError::Closed),
        (DeviceRegisters::new(2)?.extent(), AccessError::Unmapped),
        (usize::MAX, AccessError::Unmapped),
    ] {
        let mut invoked = false;
        let result = windows.write(address, &[0; 4], |_, _| {
            invoked = true;
            Ok::<_, ()>(())
        });
        // Then: no rejected access invokes downstream.
        assert_eq!(result, Err(expected));
        assert!(!invoked);
    }
    Ok(())
}

#[test]
fn closing_one_gate_keeps_peer_status_and_global_access() -> TestResult {
    // Given: both gates are open and their statuses are independently addressed.
    let mut windows = ChannelWindows::new(2)?;
    for index in 0..2 {
        let control = usize::try_from(ChannelId::new(index).window_control_address().unwrap())?;
        windows.management_write(control, &1_u32.to_le_bytes())?;
    }
    // When: firmware revokes channel zero.
    windows.management_write(usize::try_from(WINDOW_CONTROL_BASE)?, &0_u32.to_le_bytes())?;
    // Then: status zero closes, status one remains open, and global registers remain reachable.
    for (index, expected) in [(0, 0_u32), (1, 1)] {
        let status = usize::try_from(ChannelId::new(index).window_status_address().unwrap())?;
        let mut bytes = [0xa5; 4];
        windows.read::<std::convert::Infallible>(status, &mut bytes, |_, _| {
            panic!("status reached downstream")
        })?;
        assert_eq!(bytes, expected.to_le_bytes());
        windows.management_read(
            usize::try_from(ChannelId::new(index).window_control_address().unwrap())?,
            &mut bytes,
        )?;
        assert_eq!(bytes, expected.to_le_bytes());
    }
    let mut seen = None;
    windows.write(0, &7_u32.to_le_bytes(), |address, bytes| {
        seen = Some((address, bytes.to_vec()));
        Ok::<_, std::convert::Infallible>(())
    })?;
    assert_eq!(seen, Some((0, 7_u32.to_le_bytes().to_vec())));
    Ok(())
}

#[test]
fn management_and_status_cannot_be_changed_by_host() -> TestResult {
    // Given: two open gates with protected control and readiness addresses.
    let mut windows = ChannelWindows::new(2)?;
    for index in 0..2 {
        let channel = ChannelId::new(index);
        let control = usize::try_from(channel.window_control_address().unwrap())?;
        let status = usize::try_from(channel.window_status_address().unwrap())?;
        windows.management_write(control, &1_u32.to_le_bytes())?;
        // When: Host writes management and status or reads the firmware control.
        for address in [control, status] {
            let result = windows.write(address, &[0; 4], |_, _| {
                panic!("protected write reached downstream")
            });
            // Then: every protected write fails before downstream invocation.
            assert_eq!(result, Err(AccessError::<()>::Denied));
        }
        let read = windows.read(control, &mut [0; 4], |_, _| {
            panic!("control read reached downstream")
        });
        assert_eq!(read, Err(AccessError::<()>::Denied));
        assert!(windows.management_write(status, &[0; 4]).is_err());
        assert!(
            windows
                .management_write(control, &3_u32.to_le_bytes())
                .is_err()
        );
        let mut status_bytes = [0; 4];
        windows.read(status, &mut status_bytes, |_, _| {
            Ok::<_, std::convert::Infallible>(())
        })?;
        assert_eq!(status_bytes, 1_u32.to_le_bytes());
    }
    Ok(())
}

#[test]
fn invalid_counts_are_rejected_before_window_allocation() {
    // Given: empty, overflowing and reserved-space-overlapping counts.
    for count in [0, usize::MAX, 1_048_545] {
        // When: constructing the hardware permission bank.
        let result = ChannelWindows::new(count);
        // Then: construction fails with a typed configuration error.
        assert!(result.is_err());
    }
}

#[test]
fn interior_holes_keep_channel_permission_precedence() -> TestResult {
    // Given: a closed channel with padding between its command and response maps.
    let mut windows = ChannelWindows::new(3)?;
    let address = ChannelId::new(1).register_offset().unwrap() + 20;
    let mut output = [0xa5; 4];
    // When: Host reads that interior hole before and after firmware opens the channel.
    let closed = windows.read(address, &mut output, |_, _| Ok::<_, ()>(()));
    windows.management_write(
        usize::try_from(ChannelId::new(1).window_control_address().unwrap())?,
        &1_u32.to_le_bytes(),
    )?;
    let mut downstream =
        RegisterWindow::<ChannelBlock>::new(ChannelId::new(1).register_offset().unwrap());
    let opened = windows.read(address, &mut output, |a, b| downstream.read(a, b));
    // Then: permission precedes decoding; both errors preserve the caller's destination.
    assert_eq!(closed, Err(AccessError::Closed));
    assert!(matches!(opened, Err(AccessError::Downstream(_))));
    assert_eq!(output, [0xa5; 4]);
    Ok(())
}

#[test]
fn padding_access_overlapping_status_remains_denied() -> TestResult {
    // Given: two permission slots with padding before each protected status register.
    let windows = ChannelWindows::new(2)?;
    for index in 0..2 {
        let address = usize::try_from(ChannelId::new(index).window_control_address().unwrap())? + 4;
        let mut output = [0xa5; 8];
        // When: reads and writes start in padding and cross into status.
        let read = windows.read(address, &mut output, |_, _| panic!("protected read"));
        let write = windows.write(address, &[0; 8], |_, _| panic!("protected write"));
        // Then: protection wins over the unmapped starting byte, without touching output.
        assert_eq!(read, Err(AccessError::<()>::Denied));
        assert_eq!(write, Err(AccessError::<()>::Denied));
        assert_eq!(output, [0xa5; 8]);
    }
    Ok(())
}
