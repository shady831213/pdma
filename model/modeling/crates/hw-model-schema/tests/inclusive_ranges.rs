//! Inclusive bit ranges share the existing codecs and field metadata.

use hw_model_schema::{bits_schema, record_schema, register_schema};

bits_schema! {
    pub bits ClosedFlags(u16) {
        low: 0..=0,
        mode: 7..=9 => enum { variants: { Idle = 0, Ready = 5 } },
        high: 15..=15,
    }
}

record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            flags: bits(ClosedFlags),
            inline: bits(u16) { fields: { low: 0..1, mode: 1..=3 } },
        },
    }
}

register_schema! {
    pub register Control {
        width: 2, endian: Big, access: ReadWrite, reset: Zero(0), writable_mask: 0x8381,
        fields: { low: 0..=0, mode: 7..=9, high: 15..=15 },
    }
}

bits_schema! { pub bits FullWord(u64) { value: 0..=63 } }
record_schema! { pub record WordPacket { endian: Little, fields: { word: bits(FullWord) } } }

#[test]
fn records_encode_named_and_inline_closed_ranges() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let packet = Packet {
        flags: ClosedFlagsValues {
            low: true,
            mode: ClosedFlagsMode::Ready,
            high: true,
        },
        inline: PacketInlineValues { low: true, mode: 5 },
    };
    // When
    let bytes = packet.to_bytes()?;
    // Then
    assert_eq!(bytes, [0x81, 0x82, 0x0b, 0]);
    assert_eq!(Packet::from_bytes(&bytes)?, packet);
    Ok(())
}

#[test]
fn register_views_preserve_bit_positions_in_big_endian() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let mut bytes = [0; 2];
    // When
    let mut view = ControlViewMut::new(&mut bytes)?;
    view.set_low(true)?;
    view.set_mode(5)?;
    view.set_high(true)?;
    // Then
    assert_eq!(bytes, [0x82, 0x81]);
    let fields = Control::SPEC.fields();
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.lsb(), field.bit_width()))
            .collect::<Vec<_>>(),
        [(0, 1), (7, 3), (15, 1)]
    );
    Ok(())
}

#[test]
fn complete_u64_range_encodes_every_bit() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let packet = WordPacket {
        word: FullWordValues { value: u64::MAX },
    };
    // When
    let bytes = packet.to_bytes()?;
    // Then
    assert_eq!(bytes, [255; 8]);
    assert_eq!(WordPacket::from_bytes(&bytes)?, packet);
    Ok(())
}
