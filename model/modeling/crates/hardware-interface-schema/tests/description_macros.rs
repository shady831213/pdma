//! Documentation propagation through generated schemas and codecs.

use hardware_interface_schema::{bits_schema, record_schema, register_map, register_schema};

bits_schema! {
    #[short_description = "Shared flags"]
    #[description = r"Shared details.
Second line."]
    pub bits Flags(u8) {
        #[short_description = "Enabled bit"]
        #[description = "Set to enable copying."]
        enabled: 0..1,
    }
}
register_schema! {
    #[short_description = "Register definition"]
    #[description = "Register detail"]
    pub register Control {
        width: 1, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1,
        fields: Flags,
    }
}
register_schema! {
    pub register Status {
        width: 1, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: { #[description = "Ready to copy"] ready: 0..1 },
    }
}
register_map! {
    #[description = "Channel map"]
    pub map Channel { alignment: 1, placements: [
        #[short_description = "Control placement"]
        pub control: Control,
        pub status: Status,
    ] }
}
register_map! {
    #[short_description = "Device map"]
    pub map Device { alignment: 1, placements: [
        #[description = "Repeated channel placement"]
        pub channels: Channel[2],
        pub direct: Control,
    ] }
}
record_schema! {
    #[short_description = "Child record"]
    #[description = "Child details"]
    pub record Child { endian: Little, fields: {
        #[description = "Payload count"] count: u8,
    } }
}
record_schema! {
    #[description = "Outer record"]
    pub record Packet { endian: Little, fields: {
        #[short_description = "Child placement"] child: Child,
        #[short_description = "Flag placement"] flags: bits(Flags),
        #[description = "Inline flag storage"] state: bits(u8) { fields: {
            #[description = "Inline ready bit"] ready: 0..1,
        } },
    } }
}

#[test]
fn placement_descriptions_override_each_attribute_independently() {
    // Given a repeated nested map and a standalone register placement.
    // When the map expands into leaf registers.
    let resolved = Device::resolve().expect("valid map");
    // Then leaf overrides and declaration fallbacks remain distinct.
    assert_eq!(
        resolved.documentation().short_description,
        Some("Device map")
    );
    let control = resolved.registers().first().expect("control");
    assert_eq!(
        control.documentation().short_description,
        Some("Control placement")
    );
    assert_eq!(control.documentation().description, Some("Register detail"));
    assert_eq!(
        Device::DIRECT.documentation().short_description,
        Some("Register definition")
    );
    assert_eq!(Channel::STATUS.documentation().description, None);
    assert_eq!(
        Control::SPEC
            .fields()
            .first()
            .expect("enabled field")
            .documentation()
            .description,
        Some("Set to enable copying.")
    );
    assert_eq!(
        Status::SPEC
            .fields()
            .first()
            .expect("ready field")
            .documentation()
            .description,
        Some("Ready to copy")
    );
}

#[test]
fn record_embedding_preserves_storage_and_bit_member_descriptions() {
    // Given nested records plus reusable and inline bits with separate documentation.
    // When generated metadata is queried through the existing specifications.
    let child = Packet::RECORDS[0];
    let flags = Packet::FIELDS[0];
    let state = Packet::FIELDS[1];
    // Then placement text falls back per attribute and embedded bit text survives.
    assert_eq!(
        Packet::SPEC.documentation().description,
        Some("Outer record")
    );
    assert_eq!(
        child.documentation().short_description,
        Some("Child placement")
    );
    assert_eq!(child.documentation().description, Some("Child details"));
    assert_eq!(
        flags.documentation().short_description,
        Some("Flag placement")
    );
    assert_eq!(
        flags.documentation().description,
        Some("Shared details.\nSecond line.")
    );
    assert_eq!(
        flags
            .bits()
            .first()
            .expect("enabled bit")
            .documentation()
            .short_description,
        Some("Enabled bit")
    );
    assert_eq!(
        state
            .bits()
            .first()
            .expect("ready bit")
            .documentation()
            .description,
        Some("Inline ready bit")
    );
}

#[test]
fn nested_map_descriptions_remain_attached_to_their_containers() {
    // Given a repeated channel map with a container-specific override.
    // When its nested placements are resolved.
    let resolved = Device::resolve().expect("valid map");
    let placements = resolved.placements();
    // Then each container retains its path and leaf lookups retain their own text.
    assert_eq!(placements.len(), 2);
    assert_eq!(
        placements.first().expect("first channel").path(),
        "Device.channels[0]"
    );
    assert_eq!(
        placements.get(1).expect("second channel").path(),
        "Device.channels[1]"
    );
    assert_eq!(
        placements
            .first()
            .expect("first channel")
            .documentation()
            .description,
        Some("Repeated channel placement")
    );
    assert_eq!(
        Channel::register(ChannelRegisterId::Control)
            .expect("control")
            .documentation()
            .short_description,
        Some("Control placement")
    );
}

#[test]
fn documented_values_keep_their_original_codec_behavior() {
    // Given a typed value assembled through the generated constructors.
    let value = Packet::new(
        Child::new(7),
        FlagsValues::new(true),
        PacketStateValues { ready: true },
    );
    // When it is encoded and decoded through the public schema API.
    let bytes = value.to_bytes().expect("encode");
    let decoded = Packet::from_bytes(&bytes).expect("decode");
    // Then descriptions do not change scalar or semantic values.
    assert_eq!(bytes, [7, 1, 1]);
    assert_eq!(decoded, value);
}
