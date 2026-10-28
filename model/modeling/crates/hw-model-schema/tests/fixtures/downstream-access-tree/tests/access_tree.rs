#![allow(missing_docs, reason = "external fixture test")]

use his::{register_map, register_schema};

register_schema! {
    pub register ConsumerControl {
        width: 2, alignment: 4, endian: Little, access: ReadWrite,
        reset: Constant(0xa103), writable_mask: 0xff0f,
        fields: { enable: 0..1, mode: 1..4, tag: 8..16 },
    }
}

register_map! {
    pub map ConsumerMap {
        alignment: 8,
        extent: 24,
        placements: [controls: ConsumerControl[2] { offset: 8 }],
        access: [queues.command.control => controls],
    }
}

#[test]
fn typed_group_and_flat_leaf_alias_the_same_literal_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a generated downstream map backed by one literal byte image.
    let mut bytes = [0_u8; 24];
    bytes[12..14].copy_from_slice(&[0x03, 0xa1]);

    // When: a nested typed field writes the second repeated control.
    ConsumerMapViewMut::new(&mut bytes)?
        .queues_mut()?
        .command_mut()?
        .control_mut(1)?
        .set_mode(5)?;

    // Then: grouped and flat access report 0xa10b and the raw bytes are [0b,a1].
    let root = ConsumerMapView::new(&bytes)?;
    let grouped = root.queues()?.command()?.control(1)?.value()?;
    let flat = root.controls(1)?.value()?;
    assert_eq!(grouped, 0xa10b);
    assert_eq!(flat, grouped);
    assert_eq!(&bytes[12..14], &[0x0b, 0xa1]);
    println!("access_tree grouped=0xa10b flat=0xa10b bytes=[0b,a1] offset=12");
    Ok(())
}
