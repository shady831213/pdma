use hardware_interface_schema::{
    bits_schema,
    export::backends::c_header::{RecordCHeaderExporter, RegisterMapCHeaderExporter},
    record_schema, register_map, register_schema,
};

use super::support::{TestResult, compile_fragments, render};

register_schema! {
    #[short_description = "Status word"]
    pub register Status {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

register_map! {
    #[short_description = "Reusable channel map"]
    pub map Channels {
        alignment: 4,
        placements: [ status: Status { count: 2 } ],
    }
}

register_map! {
    pub map Device {
        alignment: 4,
        placements: [
            #[short_description = "Primary channel group"]
            primary: Channels,
            #[description = "Repeated auxiliary channels"]
            auxiliary: Channels { count: 2 },
        ],
    }
}

bits_schema! {
    #[short_description = "Reusable flags"]
    pub bits Flags(u32) {
        #[description = "Completion has been published."]
        done: 0..1,
    }
}

record_schema! {
    #[description = "Response payload"]
    pub record Response {
        endian: Little,
        fields: {
            #[short_description = "Response flags"]
            flags: bits(Flags),
        },
    }
}

record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            #[short_description = "Returned response"]
            response: Response,
        },
    }
}

#[test]
fn nested_map_descriptions_survive_macro_resolution_and_export() -> TestResult {
    // Given: one map reused singly and as a repeated placement.
    let resolved = Device::resolve()?;
    // When: its complete resolved structure is exported.
    let artifact = render(
        &RegisterMapCHeaderExporter::new("device.h", "device"),
        &resolved,
    )?;
    let text = std::str::from_utf8(&artifact.contents)?;
    // Then: container comments retain ownership and all six leaves retain their own text.
    assert!(text.contains("Primary channel group"));
    assert!(text.contains("Repeated auxiliary channels"));
    assert!(text.contains("Reusable channel map"));
    assert_eq!(text.matches("Status word").count(), 6);
    compile_fragments(
        "nested-map-descriptions",
        &[&artifact.contents],
        b"int main(void) { return C_DEVICE_SIZE != 24; }\n",
    )?;
    Ok(())
}

#[test]
fn nested_record_and_shared_bit_descriptions_reach_c_comments() -> TestResult {
    // Given: a nested record containing a reusable bits declaration.
    let resolved = Packet::SPEC.resolve()?;
    // When: the public exporter renders that composed schema.
    let artifact = render(&RecordCHeaderExporter::new("packet.h", "packet"), &resolved)?;
    let text = std::str::from_utf8(&artifact.contents)?;
    // Then: placement, inherited type and bit member descriptions all survive.
    for description in [
        "Returned response",
        "Response payload",
        "Response flags",
        "Completion has been published.",
    ] {
        assert!(text.contains(description), "missing {description}");
    }
    compile_fragments(
        "nested-record-descriptions",
        &[&artifact.contents],
        b"int main(void) { return C_PACKET_SIZE != 4; }\n",
    )?;
    Ok(())
}

#[test]
fn ordered_repeated_registers_are_present_in_the_export() -> TestResult {
    // Given: a generated map with two instances, not legacy bank templates.
    let resolved = Channels::resolve()?;
    // When: the public C exporter consumes that map.
    let artifact = render(
        &RegisterMapCHeaderExporter::new("channels.h", "channels"),
        &resolved,
    )?;
    let text = std::str::from_utf8(&artifact.contents)?;
    // Then: each expanded register has a declaration at its own address.
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("_OFFSET UINT64_C("))
            .count(),
        2
    );
    assert!(text.contains("_OFFSET UINT64_C(0)"));
    assert!(text.contains("_OFFSET UINT64_C(4)"));
    compile_fragments(
        "ordered-descriptions",
        &[&artifact.contents],
        b"int main(void) { return 0; }\n",
    )?;
    Ok(())
}
