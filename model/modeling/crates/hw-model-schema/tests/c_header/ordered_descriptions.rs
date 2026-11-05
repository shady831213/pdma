use crate::output_support::MemoryOutput;
use hw_model_schema::analysis::AnalyzedSchema;

use hw_model_schema::{
    bits_schema, export::backends::c_header::CRegisterBackend, record_schema, register_map,
    register_schema,
};

use super::support::{TestResult, compile_fragments, render_map, render_record};

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
        placements: [ status: Status[2] ],
    }
}

register_map! {
    pub map Device {
        alignment: 4,
        placements: [
            #[short_description = "Primary channel group"]
            primary: Channels,
            #[description = "Repeated auxiliary channels"]
            auxiliary: Channels[2],
        ],
    }
}

register_map! {
    pub map DeeplyNestedDeviceRegisterInterface {
        alignment: 4,
        placements: [
            channels_with_independent_configuration: Device[2],
        ],
    }
}

#[test]
fn long_rust_paths_export_when_actual_c_symbols_are_distinct() -> TestResult {
    // Given: repeated nesting whose Rust paths share more than 63 encoded bytes.
    let resolved = DeeplyNestedDeviceRegisterInterface::resolve()?;
    // When: rendering complete structural paths with deterministic C stem abbreviation.
    let artifact = render_map("deep.h", "deep", &resolved)?;
    let subset = resolved.project_access(|register| {
        (register.offset() == 44).then_some(hw_model_schema::Access::ReadOnly)
    })?;
    let projected = render_map("deep-subset.h", "deep", &subset)?;
    let declaration = "C_DEEP_C_CHANNELSQSWIT_B3B1D0CC33E9931B_OFFSET UINT64_C(44)";
    assert!(std::str::from_utf8(&artifact.bytes)?.contains(declaration));
    assert!(std::str::from_utf8(&projected.bytes)?.contains(declaration));
    compile_fragments(
        "deep-subset",
        &[&projected.bytes],
        b"int main(void) { return C_DEEP_C_CHANNELSQSWIT_B3B1D0CC33E9931B_OFFSET != 44; }\n",
    )?;
    // Then: the actual symbols compile and expose the final repeated register.
    compile_fragments("deep-map", &[&artifact.bytes],
        b"_Static_assert(C_DEEP_C_CHANNELSQSWIT_B3B1D0CC33E9931B_OFFSET == 44, \"last register\");\nint main(void) { return C_DEEP_SIZE != 48; }\n")?;
    Ok(())
}

#[test]
fn long_actual_c_prefix_collisions_are_rejected_before_emission() -> TestResult {
    // Given: a prefix that consumes C11's complete significant identifier length.
    let resolved = Channels::resolve()?;
    let exporter = ("long.h", "a".repeat(64));
    let mut sink = MemoryOutput::new();
    // When: rendering symbols whose distinguishing suffix lies beyond that length.
    let schema = AnalyzedSchema::register_map(exporter.0, &resolved)?;
    let result = sink.generate(
        &schema,
        CRegisterBackend::new().with_name(
            schema.view(exporter.0).ok_or("missing view")?.id.clone(),
            exporter.1.as_ref(),
        ),
    );
    // Then: no artifact is emitted and the collision remains a typed error.
    assert!(matches!(
        result,
        Err(hw_model_schema::export::ExportError::Render(
            hw_model_schema::export::backends::c_header::CHeaderExportError::NameCollision { .. }
        ))
    ));
    assert!(sink.outputs().is_empty());
    Ok(())
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
    let artifact = render_map("device.h", "device", &resolved)?;
    let text = std::str::from_utf8(&artifact.bytes)?;
    // Then: container comments retain ownership and all six leaves retain their own text.
    assert!(text.contains("Primary channel group"));
    assert!(text.contains("Repeated auxiliary channels"));
    assert!(text.contains("Reusable channel map"));
    assert_eq!(text.matches("Status word").count(), 6);
    compile_fragments(
        "nested-map-descriptions",
        &[&artifact.bytes],
        b"int main(void) { return C_DEVICE_SIZE != 24; }\n",
    )?;
    Ok(())
}

#[test]
fn nested_record_and_shared_bit_descriptions_reach_c_comments() -> TestResult {
    // Given: a nested record containing a reusable bits declaration.
    let resolved = Packet::SPEC;
    // When: the public exporter renders that composed schema.
    let artifact = render_record("packet.h", "packet", &resolved)?;
    let text = std::str::from_utf8(&artifact.bytes)?;
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
        &[&artifact.bytes],
        b"int main(void) { return C_PACKET_SIZE != 4; }\n",
    )?;
    Ok(())
}

#[test]
fn ordered_repeated_registers_are_present_in_the_export() -> TestResult {
    // Given: a generated map with two instances, not legacy bank templates.
    let resolved = Channels::resolve()?;
    // When: the public C exporter consumes that map.
    let artifact = render_map("channels.h", "channels", &resolved)?;
    let text = std::str::from_utf8(&artifact.bytes)?;
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
        &[&artifact.bytes],
        b"int main(void) { return 0; }\n",
    )?;
    Ok(())
}
