use super::{
    CHeaderExportError, CHeaderRenderer, fragment::Fragment, naming, record, register_map,
};
use crate::{
    Documentation, EnumVariantSpec, FieldDefinition, RecordSpec, ResolvedMember, ResolvedRecord,
    export::{
        Artifact, BankView, BitField, ChildCall, DefinitionHeader, EnumSite, ExportId, FieldOwner,
        GeometryAssignment, GeometryMember, GeometrySignature, RegisterLocation, RegisterMapView,
        RegisterNode, Renderer,
    },
    graph::{BitsDefinition, MapLayoutPlan},
};

type RenderResult<T = ()> = Result<T, CHeaderExportError>;

impl Renderer for CHeaderRenderer {
    type Error = CHeaderExportError;

    fn begin_definition(&mut self, definition: DefinitionHeader<'_>) -> RenderResult {
        (self.logical_name, self.prefix) = self.definition_names(definition)?;
        self.output = Fragment::default();
        Ok(())
    }

    fn end_definition(&mut self, _: DefinitionHeader<'_>) -> RenderResult {
        self.artifacts.push(Artifact {
            logical_name: self.logical_name.clone(),
            media_type: "text/x-c; charset=utf-8".into(),
            contents: std::mem::take(&mut self.output.text).into_bytes(),
        });
        Ok(())
    }

    fn begin_record(&mut self, schema: &ResolvedRecord) -> RenderResult {
        naming::check_record(schema)?;
        self.record_root = schema.root();
        self.output.documentation("", schema.documentation())?;
        self.output.constants::<64>(
            &self.prefix,
            &[("SIZE", schema.size()), ("ALIGNMENT", schema.alignment())],
        )?;
        self.output
            .constants::<32>(&self.prefix, &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)])
    }

    fn record_group(&mut self, member: &ResolvedMember, _: &RecordSpec) -> RenderResult {
        self.output
            .documentation(member.path(), member.documentation())
    }

    fn begin_record_field(
        &mut self,
        _: usize,
        member: &ResolvedMember,
        _: FieldDefinition,
    ) -> RenderResult {
        self.output.text.push('\n');
        self.output.documentation("", member.documentation())
    }

    fn end_record_field(
        &mut self,
        _: usize,
        member: &ResolvedMember,
        field: FieldDefinition,
    ) -> RenderResult {
        let name = self.record_member_name(member);
        let metadata = format!("{}_{name}", self.prefix);
        self.output
            .constants::<64>(&metadata, &[("OFFSET", member.absolute_offset())])?;
        let names = record::FieldNames {
            prefix: &self.prefix,
            field: &name,
            metadata: &metadata,
        };
        record::scalar_accessors(&mut self.output, names, field)
    }

    fn record_placement(
        &mut self,
        member: &ResolvedMember,
        definition: &RecordSpec,
    ) -> RenderResult {
        let name = format!("{}_RECORD_{}", self.prefix, self.record_member_name(member));
        self.output.constants::<64>(
            &name,
            &[
                ("OFFSET", member.absolute_offset()),
                ("SIZE", definition.size()),
            ],
        )
    }

    fn begin_bits(&mut self, layout: &BitsDefinition) -> RenderResult {
        self.output
            .constants::<64>(&self.prefix, &[("SIZE", layout.scalar().width())])?;
        self.output
            .constants::<32>(&self.prefix, &[("BIT_WIDTH", layout.bit_width())])?;
        self.output
            .constants::<64>(&self.prefix, &[("DECLARED_MASK", layout.declared_mask())])
    }

    fn bit_field(&mut self, field: BitField<'_>) -> RenderResult {
        let Some(prefix) = self.field_owner_prefix(field.owner)? else {
            return Ok(());
        };
        match field.owner {
            FieldOwner::Bits => self.output.constants::<32>(
                &format!("{prefix}_{}", naming::c_identifier(field.name)),
                &[("LSB", field.lsb), ("BITS", field.width)],
            ),
            FieldOwner::Record { .. } | FieldOwner::Register(_) => {
                self.output.documentation(field.name, field.documentation)
            }
        }
    }

    fn enum_variant(&mut self, site: EnumSite<'_>, variant: EnumVariantSpec) -> RenderResult {
        let Some(name) = self.enum_name(site, variant.name())? else {
            return Ok(());
        };
        self.output
            .documentation(variant.name(), variant.documentation())?;
        self.output.unsigned::<64>(&name, variant.value())
    }

    fn begin_map_instance(&mut self, map: &RegisterMapView) -> RenderResult {
        self.map_names = register_map::map_names(map);
        self.output.documentation("", map.documentation)?;
        self.map_constants(map.extent)
    }

    fn map_group(&mut self, path: &str, documentation: Documentation) -> RenderResult {
        self.output.documentation(path, documentation)
    }

    fn begin_bank(&mut self, index: usize, bank: &BankView, expanded: bool) -> RenderResult {
        let name = self.bank_name(index)?;
        self.output.text.push('\n');
        if expanded {
            self.output.constants::<64>(&name, &[("INDEX", index)])?;
            if let Some((parent, instance)) = bank.parent {
                self.output.constants::<64>(
                    &name,
                    &[("PARENT_INDEX", parent), ("PARENT_INSTANCE", instance)],
                )?;
            }
        } else {
            self.output.documentation("", bank.documentation)?;
        }
        register_map::bank_metadata(&mut self.output, &name, bank)
    }

    fn begin_register(&mut self, node: RegisterNode<'_>) -> RenderResult {
        match node.location {
            RegisterLocation::Root => {
                self.output
                    .documentation("", node.register.documentation())?;
                self.map_constants(node.register.width())
            }
            RegisterLocation::Standalone(_) | RegisterLocation::Template { .. } => {
                if matches!(node.location, RegisterLocation::Standalone(_)) {
                    self.output.text.push('\n');
                }
                self.output.documentation(
                    "",
                    node.view
                        .ok_or(CHeaderExportError::InvalidVisit("register view"))?
                        .documentation,
                )
            }
            RegisterLocation::Instance { .. } => Ok(()),
        }
    }

    fn end_register(&mut self, node: RegisterNode<'_>) -> RenderResult {
        let offset = self.register_offset(node)?;
        if matches!(node.location, RegisterLocation::Instance { .. }) {
            let (name, offset) =
                offset.ok_or(CHeaderExportError::InvalidVisit("instance offset"))?;
            return self.output.unsigned::<64>(&name, offset);
        }
        let name = self.register_name(node.location)?;
        let names = register_map::RegisterNames {
            prefix: &self.prefix,
            name: &name,
            offset: offset
                .as_ref()
                .map(|(name, offset)| (name.as_str(), *offset)),
        };
        register_map::push_register(&mut self.output, names, node.register)
    }

    fn begin_geometry(&mut self, _: &MapLayoutPlan) -> RenderResult {
        self.geometry_names = naming::GeometryNames::new(&self.prefix)?;
        self.output.line("typedef struct {")
    }

    fn geometry_member(&mut self, member: GeometryMember<'_>) -> RenderResult {
        let name = naming::geometry_member_name(member);
        self.geometry_names.members.insert(&name, &name)?;
        self.output.line(format_args!("    uint64_t {name};"))
    }

    fn end_geometry_members(&mut self) -> RenderResult {
        self.output
            .line(format_args!("}} {}_Geometry;", self.prefix))
    }

    fn geometry_parameter(&mut self, name: &str) -> RenderResult<String> {
        let name = format!("p_{}", naming::c_identifier(name));
        self.geometry_names.parameters.insert(&name, &name)?;
        Ok(format!("uint64_t {name}"))
    }

    fn begin_geometry_body(
        &mut self,
        _: &MapLayoutPlan,
        signature: &GeometrySignature,
    ) -> RenderResult {
        let parameters = if signature.parameters.is_empty() {
            "void".into()
        } else {
            signature.parameters.join(", ")
        };
        let prefix = &self.prefix;
        self.output.line(format_args!("static inline {prefix}_Geometry {prefix}_GEOMETRY({parameters}) {{\n    {prefix}_Geometry result = {{0}};"))?;
        if !signature.parameters.is_empty() {
            self.output.text.push_str("    uint64_t cursor = 0;\n");
        }
        Ok(())
    }

    fn geometry_child_call(&mut self, call: ChildCall<'_>, arguments: &[String]) -> RenderResult {
        let name = naming::c_identifier(call.name);
        let target = self.target_prefix(
            &ExportId::Definition(call.definition),
            &call.definition_name.to_ascii_lowercase(),
        );
        self.output.line(format_args!(
            "    {target}_Geometry child_{name} = {target}_GEOMETRY({});",
            arguments.join(", ")
        ))
    }

    fn geometry_assignment(
        &mut self,
        assignment: GeometryAssignment<'_>,
        expression: &str,
    ) -> RenderResult {
        let target = match assignment {
            GeometryAssignment::Member(member) => {
                format!("result.{}", naming::geometry_member_name(member))
            }
            GeometryAssignment::End(name) => format!("uint64_t end_{}", naming::c_identifier(name)),
            GeometryAssignment::Cursor => "cursor".into(),
        };
        self.output
            .line(format_args!("    {target} = {expression};"))
    }

    fn end_geometry_body(&mut self) -> RenderResult {
        self.output.line("    return result;\n}")
    }
    fn expression_constant(&mut self, value: usize) -> String {
        format!("UINT64_C({value})")
    }
    fn expression_identifier(&mut self, name: &str) -> String {
        naming::c_identifier(name)
    }
    fn finish(self) -> RenderResult<Vec<Artifact>> {
        Ok(self.artifacts)
    }
}
