use super::{
    VerylBackend, VerylExportError, identifier,
    package::{Package, prefix},
};
use crate::{
    BitsFieldSpec, EnumVariantSpec, RecordSpec,
    ast::{
        BitsDefinition, MapView, Node, PlacementView, RecordDefinition, RegisterInfo,
        RegisterLocation, UnionDefinition, analysis::Layout,
    },
    export::{Artifact, Backend, Scope, emit, emit_all},
};

impl Backend for VerylBackend {
    type Error = VerylExportError;

    fn definition(&mut self, node: Node<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        let name = self.name(&scope.unit.id, &scope.unit.name);
        if !identifier(&name) || self.symbols.insert(&name, scope.unit.id.clone()).is_err() {
            return Err(VerylExportError::Name(name));
        }
        self.package = Package::default();
        self.package.text = format!(
            "// Generated schema layout; do not edit.\n#[fmt(skip)]\npub package {name} {{\n"
        );
        emit(node, scope, self)?;
        self.package.text.push_str("}\n");
        self.artifacts.push(Artifact {
            logical_name: format!("{name}.veryl"),
            media_type: "text/x-veryl; charset=utf-8".into(),
            contents: std::mem::take(&mut self.package.text).into_bytes(),
        });
        Ok(())
    }

    fn record(&mut self, _: &RecordDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        self.record_extent(scope)
    }
    fn union(&mut self, _: &UnionDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        self.record_extent(scope)
    }
    fn opaque(&mut self, _: &RecordSpec, scope: Scope<'_>) -> Result<(), Self::Error> {
        self.record_extent(scope)
    }
    fn bits(&mut self, bits: &BitsDefinition, _: Scope<'_>) -> Result<(), Self::Error> {
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
    fn register_map(&mut self, _: MapView<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        if let Some(layout) = scope.map_layout() {
            if let Some(plan) = &layout.plan {
                self.geometry(plan)?;
            }
            if let Some(instance) = &layout.instance {
                self.package.constant("EXTENT", "u64", instance.extent)?;
                emit_all(MapView::Resolved(instance).children(), scope, self)?;
            }
        }
        Ok(())
    }
    fn register(&mut self, register: &RegisterInfo, scope: Scope<'_>) -> Result<(), Self::Error> {
        if scope.register_location() == RegisterLocation::Root {
            self.package
                .extent(register.width(), register.alignment())?;
        }
        if let Some(name) = prefix(scope) {
            let offset = match scope.placement {
                Some(PlacementView::Register { view, .. }) => view.offset,
                Some(PlacementView::Declared(_) | PlacementView::Bank { .. }) | None => 0,
            };
            self.package.register(&name, offset, register)?;
            emit_all(
                register.fields().iter().map(Node::RegisterField),
                scope,
                self,
            )?;
        }
        Ok(())
    }
    fn register_field(
        &mut self,
        field: &BitsFieldSpec<usize>,
        scope: Scope<'_>,
    ) -> Result<(), Self::Error> {
        if let Some(prefix) = prefix(scope) {
            let name = format!("{prefix}_{}", field.name().to_ascii_uppercase());
            self.package
                .constant(&format!("{name}_LSB"), "u32", field.lsb())?;
            self.package
                .constant(&format!("{name}_BITS"), "u32", field.bit_width())?;
        }
        emit_all(Node::RegisterField(field).children(), scope, self)
    }
    fn variant(&mut self, variant: &EnumVariantSpec, scope: Scope<'_>) -> Result<(), Self::Error> {
        if let (Some(prefix), Some(bit)) = (prefix(scope), scope.bit) {
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
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(self.artifacts)
    }
}
impl VerylBackend {
    fn record_extent(&mut self, scope: Scope<'_>) -> Result<(), VerylExportError> {
        if let Layout::Record(record) = &scope.unit.body {
            self.package.extent(record.size(), record.alignment())?;
        }
        Ok(())
    }
}
