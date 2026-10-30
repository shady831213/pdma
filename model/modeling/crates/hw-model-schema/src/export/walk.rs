pub(super) mod expression;
mod geometry;
use super::{
    BitField, DefinitionHeader, EnumSite, ExportError, FieldOwner, PreparedBody,
    PreparedDefinition, RegisterLocation, RegisterMapView, RegisterNode, Renderer,
};
use crate::{BitsFieldSpec, EnumDefinition, ResolvedMemberKind, ResolvedRecord};

type WalkResult<R, S> = Result<(), ExportError<R, S>>;

pub(super) fn definition<R: Renderer, S>(
    renderer: &mut R,
    definition: &PreparedDefinition,
) -> WalkResult<R::Error, S> {
    let header = DefinitionHeader {
        id: &definition.id,
        name: &definition.name,
        kind: definition.kind,
        documentation: definition.documentation,
    };
    renderer
        .begin_definition(header)
        .map_err(ExportError::Render)?;
    for reference in &definition.references {
        renderer.reference(reference).map_err(ExportError::Render)?;
    }
    match &definition.body {
        PreparedBody::Record(record) => record_body(renderer, record)?,
        PreparedBody::Bits(bits) => {
            renderer.begin_bits(bits).map_err(ExportError::Render)?;
            fields(renderer, FieldOwner::Bits, bits.fields())?;
            renderer.end_bits(bits).map_err(ExportError::Render)?;
        }
        PreparedBody::Register(info) => register(
            renderer,
            RegisterNode {
                location: RegisterLocation::Root,
                register: info,
                view: None,
            },
        )?,
        PreparedBody::Map(map) => {
            renderer.begin_map(map).map_err(ExportError::Render)?;
            if let Some(plan) = &map.plan {
                geometry::walk(renderer, plan)?;
            }
            if let Some(instance) = &map.instance {
                map_instance(renderer, instance)?;
            }
            renderer.end_map(map).map_err(ExportError::Render)?;
        }
    }
    renderer.end_definition(header).map_err(ExportError::Render)
}

fn record_body<R: Renderer, S>(r: &mut R, record: &ResolvedRecord) -> WalkResult<R::Error, S> {
    r.begin_record(record).map_err(ExportError::Render)?;
    for (index, member) in record.members().iter().enumerate() {
        r.record_member(index, member)
            .map_err(ExportError::Render)?;
        if let ResolvedMemberKind::Record { definition, .. } = member.kind() {
            r.record_group(member, definition)
                .map_err(ExportError::Render)?;
        }
    }
    for (index, (member, field)) in record.scalar_members().enumerate() {
        r.begin_record_field(index, member, field)
            .map_err(ExportError::Render)?;
        let owner = FieldOwner::Record { index, member };
        fields(r, owner, field.bits())?;
        enumeration(r, EnumSite { owner, bit: None }, field.enum_definition())?;
        r.end_record_field(index, member, field)
            .map_err(ExportError::Render)?;
    }
    for member in record.members() {
        if let ResolvedMemberKind::Record { definition, .. } = member.kind() {
            r.record_placement(member, definition)
                .map_err(ExportError::Render)?;
        }
    }
    r.end_record(record).map_err(ExportError::Render)
}

fn map_instance<R: Renderer, S>(r: &mut R, map: &RegisterMapView) -> WalkResult<R::Error, S> {
    r.begin_map_instance(map).map_err(ExportError::Render)?;
    for (path, docs) in &map.groups {
        r.map_group(path, *docs).map_err(ExportError::Render)?;
    }
    for (index, view) in map.registers.iter().enumerate() {
        register(
            r,
            RegisterNode {
                location: RegisterLocation::Standalone(index),
                register: &view.register,
                view: Some(view),
            },
        )?;
    }
    for (bank_index, bank) in map.banks.iter().enumerate() {
        r.begin_bank(bank_index, bank, map.expanded_banks)
            .map_err(ExportError::Render)?;
        if !map.expanded_banks {
            for (member, view) in bank.registers.iter().enumerate() {
                register(
                    r,
                    RegisterNode {
                        location: RegisterLocation::Template {
                            bank: bank_index,
                            member,
                        },
                        register: &view.register,
                        view: Some(view),
                    },
                )?;
            }
            for (instance, registers) in bank.instances.iter().enumerate() {
                for (member, view) in registers.iter().enumerate() {
                    register(
                        r,
                        RegisterNode {
                            location: RegisterLocation::Instance {
                                bank: bank_index,
                                instance,
                                member,
                            },
                            register: &view.register,
                            view: Some(view),
                        },
                    )?;
                }
            }
        }
        r.end_bank(bank_index, bank, map.expanded_banks)
            .map_err(ExportError::Render)?;
    }
    r.end_map_instance(map).map_err(ExportError::Render)
}

fn register<R: Renderer, S>(r: &mut R, node: RegisterNode<'_>) -> WalkResult<R::Error, S> {
    r.begin_register(node).map_err(ExportError::Render)?;
    fields(r, FieldOwner::Register(node), node.register.fields())?;
    r.end_register(node).map_err(ExportError::Render)
}
fn fields<R: Renderer, S, I: Copy>(
    r: &mut R,
    owner: FieldOwner<'_>,
    fields: &[BitsFieldSpec<I>],
) -> WalkResult<R::Error, S> {
    for field in fields {
        r.bit_field(BitField {
            owner,
            name: field.name(),
            lsb: field.lsb(),
            width: field.bit_width(),
            documentation: field.documentation(),
        })
        .map_err(ExportError::Render)?;
        enumeration(
            r,
            EnumSite {
                owner,
                bit: Some(field.name()),
            },
            field.definition().enum_definition(),
        )?;
    }
    Ok(())
}
fn enumeration<R: Renderer, S>(
    r: &mut R,
    site: EnumSite<'_>,
    definition: Option<EnumDefinition>,
) -> WalkResult<R::Error, S> {
    if let Some(definition) = definition {
        for variant in definition.variants() {
            r.enum_variant(site, *variant)
                .map_err(ExportError::Render)?;
        }
    }
    Ok(())
}
