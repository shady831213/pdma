use super::{
    VerylExportError, VerylRenderer, identifier,
    package::{Package, prefix},
};
use crate::{
    EnumVariantSpec, ResolvedRecord,
    export::{
        Artifact, BitField, ChildCall, DefinitionHeader, EnumSite, FieldOwner, GeometryAssignment,
        GeometryMember, GeometrySignature, PlacementProperty, RegisterLocation, RegisterMapView,
        RegisterNode, Renderer,
    },
    graph::{BitsDefinition, MapLayoutPlan},
};
use std::fmt::Write;

impl Renderer for VerylRenderer {
    type Error = VerylExportError;
    fn begin_definition(&mut self, definition: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        let name = self.name(definition.id, definition.name);
        if !identifier(&name) || self.symbols.insert(&name, definition.id.clone()).is_err() {
            return Err(VerylExportError::Name(name));
        }
        self.package = Package::default();
        self.package.text = format!(
            "// Generated schema layout; do not edit.\n#[fmt(skip)]\npub package {name} {{\n"
        );
        Ok(())
    }
    fn end_definition(&mut self, definition: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        self.package.text.push_str("}\n");
        self.artifacts.push(Artifact {
            logical_name: format!("{}.veryl", self.name(definition.id, definition.name)),
            media_type: "text/x-veryl; charset=utf-8".into(),
            contents: std::mem::take(&mut self.package.text).into_bytes(),
        });
        Ok(())
    }
    fn begin_record(&mut self, record: &ResolvedRecord) -> Result<(), Self::Error> {
        self.package.extent(record.size(), record.alignment())
    }
    fn begin_bits(&mut self, bits: &BitsDefinition) -> Result<(), Self::Error> {
        self.package
            .extent(bits.scalar().width(), bits.scalar().width())?;
        self.package
            .constant("BIT_WIDTH", "u32", bits.bit_width())?;
        self.package.constant(
            "DECLARED_MASK",
            "u64",
            format_args!("64'h{:016x}", bits.declared_mask()),
        )
    }
    fn begin_map_instance(&mut self, map: &RegisterMapView) -> Result<(), Self::Error> {
        self.package.constant("EXTENT", "u64", map.extent)
    }
    fn begin_register(&mut self, node: RegisterNode<'_>) -> Result<(), Self::Error> {
        match node.location {
            RegisterLocation::Root => self
                .package
                .extent(node.register.width(), node.register.alignment())?,
            RegisterLocation::Standalone(_)
            | RegisterLocation::Template { .. }
            | RegisterLocation::Instance { .. } => {}
        }
        if let Some(name) = prefix(node) {
            self.package.register(
                &name,
                node.view.map_or(0, |view| view.offset),
                node.register,
            )?;
        }
        Ok(())
    }
    fn bit_field(&mut self, field: BitField<'_>) -> Result<(), Self::Error> {
        match field.owner {
            FieldOwner::Register(node) => {
                if let Some(prefix) = prefix(node) {
                    let name = format!("{prefix}_{}", field.name.to_ascii_uppercase());
                    self.package
                        .constant(&format!("{name}_LSB"), "u32", field.lsb)?;
                    self.package
                        .constant(&format!("{name}_BITS"), "u32", field.width)?;
                }
            }
            FieldOwner::Bits | FieldOwner::Record { .. } => {}
        }
        Ok(())
    }
    fn enum_variant(
        &mut self,
        site: EnumSite<'_>,
        variant: EnumVariantSpec,
    ) -> Result<(), Self::Error> {
        match site.owner {
            FieldOwner::Register(node) => {
                if let (Some(prefix), Some(bit)) = (prefix(node), site.bit) {
                    self.package.constant(
                        &format!(
                            "{prefix}_{}_{}",
                            bit.to_ascii_uppercase(),
                            variant.name().to_ascii_uppercase()
                        ),
                        "u64",
                        variant.value(),
                    )?;
                }
            }
            FieldOwner::Bits | FieldOwner::Record { .. } => {}
        }
        Ok(())
    }
    fn begin_geometry(&mut self, plan: &MapLayoutPlan) -> Result<(), Self::Error> {
        self.package
            .constant("ALIGNMENT", "u64", plan.alignment())?;
        self.package.declare("Geometry")?;
        self.package.declare("geometry")?;
        self.package.text.push_str("    struct Geometry {\n");
        Ok(())
    }
    fn geometry_member(&mut self, member: GeometryMember<'_>) -> Result<(), Self::Error> {
        match member {
            GeometryMember::Placement { name, .. } if !identifier(name) => {
                return Err(VerylExportError::Name(name.into()));
            }
            GeometryMember::Placement { .. } | GeometryMember::Extent => {}
        }
        writeln!(self.package.text, "        {}: u64,", member_name(member))?;
        Ok(())
    }
    fn end_geometry_members(&mut self) -> Result<(), Self::Error> {
        self.package.text.push_str("    }\n");
        Ok(())
    }
    fn geometry_parameter(&mut self, name: &str) -> Result<String, Self::Error> {
        if !identifier(&format!("p_{name}")) {
            return Err(VerylExportError::Name(name.into()));
        }
        Ok(format!("p_{name}: input u64"))
    }
    fn begin_geometry_body(
        &mut self,
        _: &MapLayoutPlan,
        signature: &GeometrySignature,
    ) -> Result<(), Self::Error> {
        write!(
            self.package.text,
            "    function geometry({}) -> Geometry {{\n        var result: Geometry;\n",
            signature.parameters.join(", ")
        )?;
        if !signature.parameters.is_empty() {
            self.package.text.push_str("        var cursor: u64;\n");
        }
        self.package.text.push_str("        result = '0;\n");
        if !signature.parameters.is_empty() {
            self.package.text.push_str("        cursor = 0;\n");
        }
        Ok(())
    }
    fn geometry_child_call(
        &mut self,
        call: ChildCall<'_>,
        arguments: &[String],
    ) -> Result<(), Self::Error> {
        let package = self.name(&call.definition.into(), call.definition_name);
        writeln!(
            self.package.text,
            "        let child_{}: {package}::Geometry = {package}::geometry({});",
            call.name,
            arguments.join(", ")
        )?;
        Ok(())
    }
    fn geometry_assignment(
        &mut self,
        assignment: GeometryAssignment<'_>,
        expression: &str,
    ) -> Result<(), Self::Error> {
        match assignment {
            GeometryAssignment::Member(member) => writeln!(
                self.package.text,
                "        result.{} = {expression};",
                member_name(member)
            )?,
            GeometryAssignment::End(name) => {
                writeln!(
                    self.package.text,
                    "        let end_{name}: u64 = {expression};"
                )?;
            }
            GeometryAssignment::Cursor => {
                writeln!(self.package.text, "        cursor = {expression};")?;
            }
        }
        Ok(())
    }
    fn end_geometry_body(&mut self) -> Result<(), Self::Error> {
        self.package
            .text
            .push_str("        return result;\n    }\n");
        Ok(())
    }
    fn geometry_accessor(
        &mut self,
        name: &str,
        property: PlacementProperty,
        signature: &GeometrySignature,
    ) -> Result<(), Self::Error> {
        let name = member_name(GeometryMember::Placement { name, property });
        self.package.declare(&name)?;
        write!(
            self.package.text,
            "    function {name}({}) -> u64 {{\n        let shape: Geometry = geometry({});\n        return shape.{name};\n    }}\n",
            signature.parameters.join(", "),
            signature.arguments.join(", ")
        )?;
        Ok(())
    }
    fn expression_maximum(&mut self, left: &str, right: &str) -> String {
        format!("(if {left} >: {right} ? {left} : {right})")
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(self.artifacts)
    }
}

fn member_name(member: GeometryMember<'_>) -> String {
    match member {
        GeometryMember::Extent => "extent".into(),
        GeometryMember::Placement { name, property } => format!("{name}_{}", property.name()),
    }
}
