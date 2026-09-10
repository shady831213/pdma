//! Documentation coverage of the concrete PDMA wire contract.

use hardware_interface_schema::Documentation;
use pdma_model_contract::{
    Address, ChannelFault, ChannelRegisters, Command, CommandRegisters, DeviceRegisters, ReadWord,
    Response, ResponseRegisters, Switch, Word,
};

fn assert_documented(name: &str, docs: Documentation) {
    assert!(
        docs.short_description
            .is_some_and(|text| !text.trim().is_empty()),
        "{name}: missing summary"
    );
    assert!(
        docs.description.is_some_and(|text| !text.trim().is_empty()),
        "{name}: missing description"
    );
}

#[test]
fn register_definitions_and_placements_have_complete_documentation()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the reusable register types and the concrete device register tree.
    let definitions = [
        Word::SPEC,
        ReadWord::SPEC,
        Address::SPEC,
        Switch::SPEC,
        ChannelFault::SPEC,
    ];
    // When: the public metadata is resolved as a documentation generator would consume it.
    let map = DeviceRegisters::resolve()?;
    // Then: both descriptions exist at every definition, bit field, map and leaf placement.
    for definition in definitions {
        assert_documented(definition.name(), definition.documentation());
        for field in definition.fields() {
            assert_documented(field.name(), field.documentation());
        }
    }
    for docs in [
        DeviceRegisters::SPEC.documentation(),
        CommandRegisters::SPEC.documentation(),
        ResponseRegisters::SPEC.documentation(),
        ChannelRegisters::SPEC.documentation(),
    ] {
        assert_documented("map definition", docs);
    }
    assert_documented(map.root(), map.documentation());
    for placement in map.placements() {
        assert_documented(placement.path(), placement.documentation());
    }
    for register in map.registers() {
        assert_documented(register.path(), register.documentation());
        assert_ne!(
            register.documentation(),
            register.definition().documentation(),
            "{} needs its own hardware meaning",
            register.path()
        );
    }
    Ok(())
}

#[test]
fn command_and_response_records_describe_every_wire_field() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: both record definitions used by the channel's rings.
    let definitions = [Command::SPEC, Response::SPEC];
    // When: their metadata is resolved through the public schema surface.
    let records = definitions
        .iter()
        .map(|definition| definition.resolve())
        .collect::<Result<Vec<_>, _>>()?;
    // Then: root and member descriptions are available without reading Rust source.
    for record in records {
        assert_documented(record.root(), record.documentation());
        for member in record.members() {
            assert_documented(member.path(), member.documentation());
        }
    }
    Ok(())
}
