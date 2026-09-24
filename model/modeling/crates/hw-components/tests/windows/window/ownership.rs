//! Single-window ownership against real downstream register storage.
use hardware_interface_schema::{RegisterAccessError, RegisterWindow, register_map};
use pdma_model_contract::hw_components::windows::{ChannelWindowBlock, WindowControl};
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::register::Word;
use pdma_model_hw_components::windows::{AccessError, ChannelWindow};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CONTROL: usize = 0x1000;
const STATUS: usize = CONTROL + ChannelWindowBlock::STATUS.offset();

register_map! {
    map Storage { alignment: 4, placements: [pub data: Word], }
}

#[test]
fn foreign_requester_cannot_access_data_or_readiness() -> TestResult {
    // Given: an open Function-7 window in front of real register storage.
    let mut window = ChannelWindow::new(0..4, CONTROL)?;
    window.management_write(
        CONTROL + WindowControl::OWNER.offset(),
        &7_u32.to_le_bytes(),
    )?;
    window.management_write(CONTROL, &1_u32.to_le_bytes())?;
    let mut storage = RegisterWindow::<Storage>::new(0);
    storage.write(0, &17_u32.to_le_bytes())?;
    // When: Function 9 attempts data writes and data/status reads, including malformed status.
    let write = window.write_for(FunctionId::new(9), (0, &42_u32.to_le_bytes()), |a, b| {
        storage.write(a, b)
    });
    for (address, width) in [(0, 4), (STATUS, 4), (STATUS, 0)] {
        let mut output = vec![0xa5; width];
        let read = window.read_for(FunctionId::new(9), (address, &mut output), |a, b| {
            storage.read(a, b)
        });
        // Then: ownership denial precedes decoding and preserves software buffers and backing.
        assert_eq!(read, Err(AccessError::Denied));
        assert_eq!(output, vec![0xa5; width]);
    }
    assert_eq!(write, Err(AccessError::Denied));
    assert_eq!(storage.values.data.raw(), 17);
    let mut output = [0; 4];
    window.read_for(FunctionId::new(7), (0, &mut output), |a, b| {
        storage.read(a, b)
    })?;
    assert_eq!(output, 17_u32.to_le_bytes());
    window.read_for(FunctionId::new(7), (STATUS, &mut output), |a, b| {
        storage.read(a, b)
    })?;
    assert_eq!(output, 1_u32.to_le_bytes());
    Ok(())
}

#[test]
fn closed_owner_observes_not_ready_but_cannot_access_data() -> TestResult {
    // Given: a closed window retaining Function 7 for accepted work.
    let mut window = ChannelWindow::new(0..4, CONTROL)?;
    window.management_write(
        CONTROL + WindowControl::OWNER.offset(),
        &7_u32.to_le_bytes(),
    )?;
    let mut storage = RegisterWindow::<Storage>::new(0);
    storage.write(0, &17_u32.to_le_bytes())?;
    let mut output = [0xa5; 4];
    // When: the owner reads readiness and attempts a data write.
    window.read_for(FunctionId::new(7), (STATUS, &mut output), |a, b| {
        storage.read(a, b)
    })?;
    let write = window.write_for(FunctionId::new(7), (0, &42_u32.to_le_bytes()), |a, b| {
        storage.write(a, b)
    });
    // Then: readiness is zero, data is inaccessible and ownership/backing stay intact.
    assert_eq!(output, [0; 4]);
    assert_eq!(write, Err(AccessError::Closed));
    assert_eq!(window.owner(), FunctionId::new(7));
    assert_eq!(storage.values.data.raw(), 17);
    Ok(())
}

#[test]
fn regrant_changes_authorized_requester_without_replacing_backing() -> TestResult {
    // Given: a closed Function-7 window with previously written register state.
    let mut window = ChannelWindow::new(0..4, CONTROL)?;
    window.management_write(
        CONTROL + WindowControl::OWNER.offset(),
        &7_u32.to_le_bytes(),
    )?;
    let mut storage = RegisterWindow::<Storage>::new(0);
    storage.write(0, &17_u32.to_le_bytes())?;
    // When: firmware reassigns the closed window to Function 9 and opens it.
    window.management_write(
        CONTROL + WindowControl::OWNER.offset(),
        &9_u32.to_le_bytes(),
    )?;
    window.management_write(CONTROL, &1_u32.to_le_bytes())?;
    // Then: the old owner loses data/status access; the new owner uses the same backing.
    for address in [0, STATUS] {
        let mut output = [0xa5; 4];
        assert_eq!(
            window.read_for(FunctionId::new(7), (address, &mut output), |a, b| storage
                .read(a, b)),
            Err(AccessError::Denied)
        );
        assert_eq!(output, [0xa5; 4]);
    }
    assert_eq!(
        window.write_for(FunctionId::new(7), (0, &42_u32.to_le_bytes()), |a, b| {
            storage.write(a, b)
        }),
        Err(AccessError::Denied)
    );
    let mut output = [0; 4];
    window.read_for(FunctionId::new(9), (0, &mut output), |a, b| {
        storage.read(a, b)
    })?;
    assert_eq!(output, 17_u32.to_le_bytes());
    window.write_for(FunctionId::new(9), (0, &23_u32.to_le_bytes()), |a, b| {
        storage.write(a, b)
    })?;
    assert_eq!(storage.values.data.raw(), 23);
    Ok(())
}

#[test]
fn live_binding_is_frozen_and_closing_retains_its_owner() -> TestResult {
    // Given: an open window bound to Function 7.
    let mut window = ChannelWindow::new(0..4, CONTROL)?;
    window.management_write(
        CONTROL + WindowControl::OWNER.offset(),
        &7_u32.to_le_bytes(),
    )?;
    window.management_write(CONTROL, &1_u32.to_le_bytes())?;
    // When: firmware tries a live rebind, then closes access for cleanup.
    let rebind = window.management_write(
        CONTROL + WindowControl::OWNER.offset(),
        &9_u32.to_le_bytes(),
    );
    window.management_write(CONTROL, &0_u32.to_le_bytes())?;
    // Then: the rejected rebind and close both preserve the accepted work's owner.
    assert_eq!(
        rebind,
        Err(RegisterAccessError::WriteForbidden {
            offset: WindowControl::OWNER.offset()
        })
    );
    assert_eq!(window.owner(), FunctionId::new(7));
    assert!(!window.is_open());
    Ok(())
}
