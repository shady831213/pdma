//! Function-specific channel access through the shared permission bank.
use hw_model_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::dma::{ChannelBlock, ChannelId};
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::windows::{AccessError, ChannelWindows};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const PERMISSIONS: usize = 0x10_0000;

#[test]
fn foreign_function_cannot_access_an_open_channel() -> TestResult {
    // Given: channel zero is granted to Function 7.
    let mut windows = ChannelWindows::new(2, 0, u32::try_from(PERMISSIONS)?)?;
    windows.management_write(
        PERMISSIONS + WindowControl::OWNER.offset(),
        &7_u32.to_le_bytes(),
    )?;
    windows.management_write(PERMISSIONS, &1_u32.to_le_bytes())?;
    let address = ChannelId::new(0).register_offset(2).unwrap();
    let mut registers = RegisterWindow::<ChannelBlock>::new(address);
    // When: another Function attempts a write and a read.
    let foreign = windows.for_function(FunctionId::new(9));
    let write = foreign.write(address, &23_u64.to_le_bytes(), |a, b| registers.write(a, b));
    let mut bytes = [0xa5; 8];
    let read = foreign.read(address, &mut bytes, |a, b| registers.read(a, b));
    // Then: neither operation reaches the channel or changes the read buffer.
    assert_eq!(write, Err(AccessError::Denied));
    assert_eq!(read, Err(AccessError::Denied));
    assert_eq!(bytes, [0xa5; 8]);
    windows
        .for_function(FunctionId::new(7))
        .read(address, &mut bytes, |a, b| registers.read(a, b))?;
    assert_eq!(bytes, [0; 8]);
    Ok(())
}

#[test]
fn binding_cannot_change_while_the_aperture_is_open() -> TestResult {
    // Given: an open Function-zero aperture.
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(PERMISSIONS)?)?;
    windows.management_write(PERMISSIONS, &1_u32.to_le_bytes())?;
    // When: firmware attempts to rebind without closing it.
    let result = windows.management_write(
        PERMISSIONS + WindowControl::OWNER.offset(),
        &1_u32.to_le_bytes(),
    );
    // Then: the write is forbidden and interrupt routing keeps its original owner.
    assert!(matches!(
        result,
        Err(RegisterAccessError::WriteForbidden { .. })
    ));
    assert_eq!(windows.owner(ChannelId::new(0)), Some(FunctionId::new(0)));
    Ok(())
}

#[test]
fn regrant_revokes_the_previous_function_but_preserves_peer_access() -> TestResult {
    // Given: two channels initially owned by Function zero.
    let mut windows = ChannelWindows::new(2, 0, u32::try_from(PERMISSIONS)?)?;
    let peer = PERMISSIONS + ChannelId::new(1).window_control_offset().unwrap();
    windows.management_write(PERMISSIONS, &1_u32.to_le_bytes())?;
    windows.management_write(peer, &1_u32.to_le_bytes())?;
    // When: firmware closes and rebinds channel zero.
    windows.management_write(PERMISSIONS, &0_u32.to_le_bytes())?;
    windows.management_write(
        PERMISSIONS + WindowControl::OWNER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    windows.management_write(PERMISSIONS, &1_u32.to_le_bytes())?;
    // Then: old access is revoked, the new owner succeeds, and the peer remains usable.
    let first = ChannelId::new(0).register_offset(2).unwrap();
    let second = ChannelId::new(1).register_offset(2).unwrap();
    assert_eq!(
        windows
            .for_function(FunctionId::new(0))
            .write(first, &[0; 4], |_, _| Ok::<_, ()>(())),
        Err(AccessError::Denied)
    );
    assert_eq!(
        windows
            .for_function(FunctionId::new(1))
            .write(first, &[0; 4], |_, _| Ok::<_, ()>(())),
        Ok(())
    );
    assert_eq!(
        windows
            .for_function(FunctionId::new(0))
            .write(second, &[0; 4], |_, _| Ok::<_, ()>(())),
        Ok(())
    );
    Ok(())
}
