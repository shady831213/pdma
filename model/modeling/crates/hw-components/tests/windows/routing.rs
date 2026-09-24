//! Transaction error ordering at trusted and Function-bound bank ingress.
use hardware_interface_schema::RegisterAccessError;
use pdma_model_contract::dma::{ChannelId, DmaRegisters};
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::windows::{AccessError, ChannelWindows};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const PERMISSIONS: u32 = 0x0010_0000;

#[test]
fn discovery_and_channel_bounds_keep_existing_behavior() -> TestResult {
    // Given: two open channels and real DMA discovery/register storage.
    let mut windows = ChannelWindows::new(2, 0, PERMISSIONS)?;
    let mut dma = pdma_model_dma::Dma::new((), 2)?;
    for index in 0..2 {
        windows.management_write(
            usize::try_from(PERMISSIONS)? + ChannelId::new(index).window_control_offset().unwrap(),
            &1_u32.to_le_bytes(),
        )?;
    }
    let mut count = [0; 4];
    let mut crossed = [0xa5; 8];
    let mut forwarded = false;
    // When: a foreign Function reads public discovery and the owner crosses two windows.
    windows.for_function(FunctionId::new(9)).read(
        DmaRegisters::DISCOVERY.offset(),
        &mut count,
        |a, b| dma.read(a, b),
    )?;
    let crossing = windows.for_function(FunctionId::new(0)).read(
        ChannelId::new(1).register_offset(2).unwrap() - 4,
        &mut crossed,
        |a, b| {
            forwarded = true;
            dma.read(a, b)
        },
    );
    // Then: discovery stays public, but neighboring grants cannot combine into one transaction.
    assert_eq!(count, 2_u32.to_le_bytes());
    assert_eq!(crossing, Err(AccessError::Unmapped));
    assert_eq!(crossed, [0xa5; 8]);
    assert!(!forwarded);
    Ok(())
}

#[test]
fn bank_preflight_preserves_empty_and_overflow_errors() -> TestResult {
    // Given: trusted bank ingress and distinct global, data and status apertures.
    let windows = ChannelWindows::new(2, 0, PERMISSIONS)?;
    let channel = ChannelId::new(0);
    let status = usize::try_from(PERMISSIONS)? + channel.window_status_offset().unwrap();
    // When: empty or overflowing transactions arrive at any of these routes.
    for (address, width) in [
        (0, 0),
        (channel.register_offset(2).unwrap(), 0),
        (status, 0),
        (usize::MAX, 4),
    ] {
        let mut forwarded = false;
        let mut output = vec![0xa5; width];
        let read = windows.read(address, &mut output, |_, _| {
            forwarded = true;
            Ok::<_, ()>(())
        });
        let write = windows.write(address, &vec![0; width], |_, _| {
            forwarded = true;
            Ok::<_, ()>(())
        });
        // Then: the transaction envelope fails before routing or downstream effects.
        assert_eq!(read, Err(AccessError::Unmapped));
        assert_eq!(write, Err(AccessError::Unmapped));
        assert_eq!(output, vec![0xa5; width]);
        assert!(!forwarded);
    }
    Ok(())
}

#[test]
fn function_routing_preserves_owner_and_status_error_priority() -> TestResult {
    // Given: a closed Function-7 channel whose owner may still read readiness.
    let mut windows = ChannelWindows::new(1, 0, PERMISSIONS)?;
    windows.management_write(
        usize::try_from(PERMISSIONS)? + WindowControl::OWNER.offset(),
        &7_u32.to_le_bytes(),
    )?;
    let status = usize::try_from(PERMISSIONS)? + ChannelId::new(0).window_status_offset().unwrap();
    let admin = DmaRegisters::new(1)?.admin().base();
    let mut forwarded = false;
    // When: owner and foreign requesters send zero-width status/admin transactions.
    let owner_status = windows
        .for_function(FunctionId::new(7))
        .read(status, &mut [], |_, _| {
            forwarded = true;
            Ok::<_, ()>(())
        });
    let foreign_status = windows
        .for_function(FunctionId::new(9))
        .read(status, &mut [], |_, _| {
            forwarded = true;
            Ok::<_, ()>(())
        });
    let owner_admin = windows
        .for_function(FunctionId::new(7))
        .write(admin, &[], |_, _| {
            forwarded = true;
            Ok::<_, ()>(())
        });
    // Then: foreign permission and admin routing precede leaf decoding; owner status reaches it.
    assert_eq!(foreign_status, Err(AccessError::Denied));
    assert_eq!(owner_admin, Err(AccessError::Denied));
    assert_eq!(
        owner_status,
        Err(AccessError::Register(RegisterAccessError::WidthMismatch {
            offset: 0,
            expected: 4,
            actual: 0
        }))
    );
    assert!(!forwarded);
    Ok(())
}
