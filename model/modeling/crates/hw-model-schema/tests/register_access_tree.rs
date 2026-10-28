//! Behavioral coverage for generated register access-tree views.

use hw_model_schema::{RegisterValueError, SchemaCodecError, register_map, register_schema};

register_schema! {
    pub register TreeControl {
        width: 2,
        alignment: 4,
        endian: Little,
        access: ReadWrite,
        reset: Constant(0xa103),
        writable_mask: 0xff0f,
        fields: { enable: 0..1, mode: 1..4, tag: 8..16 },
    }
}

register_schema! {
    pub register TreeStatus {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadOnly,
        reset: PostReset(0x4433_2211),
        writable_mask: 0,
        fields: { code: 0..8 },
    }
}

register_map! {
    pub map AccessTree {
        alignment: 8,
        extent: 32,
        placements: [
            status: TreeStatus,
            controls: TreeControl[2] { offset: 8 },
            tail: TreeControl { offset: 24 },
            direct: TreeControl { offset: 28 },
        ],
        access: [
            global.status => status,
            queues.command.control => controls,
            admin.command.tail => tail,
            direct_tail => direct,
        ],
    }
}

#[test]
fn grouped_field_views_alias_flat_bytes_both_directions() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: literal root bytes with nonzero padding, sibling bits, and two instances.
    let mut bytes = [0x5a_u8; 32];
    bytes[0..4].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
    bytes[8..10].copy_from_slice(&[0x8d, 0xb2]);
    bytes[12..14].copy_from_slice(&[0xe3, 0xc4]);
    bytes[24..26].copy_from_slice(&[0x07, 0xd6]);
    bytes[28..30].copy_from_slice(&[0x09, 0xe8]);

    // When: grouped writes and flat writes traverse the same root image.
    {
        let mut root = AccessTreeViewMut::new(&mut bytes)?;
        let mut queues: AccessTreeQueuesViewMut<'_> = root.queues_mut()?;
        let mut command: AccessTreeQueuesCommandViewMut<'_> = queues.command_mut()?;
        command.control_mut(1)?.set_mode(2)?;
    }
    AccessTreeViewMut::new(&mut bytes)?
        .tail_mut()?
        .set_enable(false)?;
    AccessTreeViewMut::new(&mut bytes)?
        .direct_tail_mut()?
        .set_mode(1)?;

    // Then: grouped/flat reads agree and unrelated bytes remain literal.
    let root = AccessTreeView::new(&bytes)?;
    let global: AccessTreeGlobalView<'_> = root.global()?;
    let queues: AccessTreeQueuesView<'_> = root.queues()?;
    let command: AccessTreeQueuesCommandView<'_> = queues.command()?;
    let admin: AccessTreeAdminView<'_> = root.admin()?;
    let admin_command: AccessTreeAdminCommandView<'_> = admin.command()?;
    assert_eq!(global.status()?.code()?, 0x11);
    assert_eq!(command.control(1)?.mode()?, 2);
    assert_eq!(root.controls(1)?.value()?, 0xc4e5);
    assert!(!admin_command.tail()?.enable()?);
    assert_eq!(root.tail()?.value()?, 0xd606);
    assert_eq!(root.direct_tail()?.value()?, root.direct()?.value()?);
    assert_eq!(root.direct_tail()?.value()?, 0xe803);
    assert_eq!(&bytes[4..8], &[0x5a; 4]);
    assert_eq!(&bytes[10..12], &[0x5a; 2]);
    assert_eq!(&bytes[14..24], &[0x5a; 10]);
    assert_eq!(&bytes[26..28], &[0x5a; 2]);
    assert_eq!(&bytes[30..32], &[0x5a; 2]);
    println!("grouped_field controls[1]=e5_c4 tail=06_d6 direct=03_e8 padding=5a");
    Ok(())
}

#[test]
fn grouped_field_mutable_views_also_read_reset_bytes() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the canonical recursively encoded reset image.
    let mut bytes = [0_u8; 32];
    AccessTreeValues::default().encode_into(&mut bytes)?;

    // When: immutable traversal is requested from the mutable access tree.
    let root = AccessTreeViewMut::new(&mut bytes)?;
    let status = root.global()?.status()?.value()?;
    let control = root.queues()?.command()?.control(0)?.value()?;

    // Then: reset values are observed without changing padding.
    assert_eq!(status, 0x4433_2211);
    assert_eq!(control, 0xa103);
    assert_eq!(&bytes[4..8], &[0; 4]);
    assert_eq!(&bytes[10..12], &[0; 2]);
    assert_eq!(&bytes[14..24], &[0; 10]);
    assert_eq!(&bytes[26..28], &[0; 2]);
    assert_eq!(&bytes[30..32], &[0; 2]);
    println!("grouped_field reset_status=44332211 reset_control=a103 padding=zero");
    Ok(())
}

#[test]
fn adverse_grouped_access_rejects_without_mutating_root_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: short and full root images containing sentinel bytes.
    let mut short = [0xa5_u8; 31];
    let short_before = short;
    let mut bytes = [0x5a_u8; 32];
    let before = bytes;
    let mut direct = [0x5a_u8; 2];
    let direct_before = direct;

    // When: short-root, maximum-index, and overflowing-field paths are attempted.
    let short_error = AccessTreeViewMut::new(&mut short).err();
    let index_error = AccessTreeViewMut::new(&mut bytes)?
        .queues_mut()?
        .command_mut()?
        .control_mut(usize::MAX)
        .err();
    let overflow_error = AccessTreeViewMut::new(&mut bytes)?
        .queues_mut()?
        .command_mut()?
        .control_mut(1)?
        .set_mode(8);
    let direct_overflow_error = TreeControlViewMut::new(&mut direct)?.set_mode(8);

    // Then: errors are exact and every root byte remains unchanged.
    assert_eq!(
        short_error,
        Some(SchemaCodecError::BufferTooShort {
            required: 32,
            actual: 31
        })
    );
    assert_eq!(
        index_error,
        Some(SchemaCodecError::BufferTooShort {
            required: 2,
            actual: usize::MAX
        })
    );
    assert_eq!(
        overflow_error,
        Err(RegisterValueError::FieldValueOverflow {
            location: TreeControl::MODE_IDENTITY.at_bit(97),
            bit_width: 3,
            value: 8,
        })
    );
    assert_eq!(
        direct_overflow_error,
        Err(RegisterValueError::FieldValueOverflow {
            location: TreeControl::MODE_IDENTITY.at_bit(1),
            bit_width: 3,
            value: 8,
        })
    );
    assert_eq!(short, short_before);
    assert_eq!(bytes, before);
    assert_eq!(direct, direct_before);
    println!(
        "adverse short=32/31 index=max/count2 grouped_overflow=bit97 direct_overflow=bit1 unchanged=true"
    );
    Ok(())
}
