use super::{CHeaderBackend, CHeaderExportError, fragment::Fragment, naming, register_map};
use crate::ast::{
    BitsDefinition, MapView, Node, PlacementView, RecordDefinition, RegisterInfo, RegisterLocation,
    UnionDefinition,
};
use crate::export::{Artifact, Backend, Scope, emit, emit_all};
use crate::{BitsFieldSpec, EnumDefinition, RecordSpec};

type CResult<T = ()> = Result<T, CHeaderExportError>;

impl Backend for CHeaderBackend {
    type Error = CHeaderExportError;
    fn definition(&mut self, node: Node<'_>, scope: Scope<'_>) -> CResult {
        (self.logical_name, self.prefix) = self.definition_names(scope.unit)?;
        self.output = Fragment::default();
        emit(node, scope, self)?;
        self.artifacts.push(Artifact {
            logical_name: self.logical_name.clone(),
            media_type: "text/x-c; charset=utf-8".into(),
            contents: std::mem::take(&mut self.output.text).into_bytes(),
        });
        Ok(())
    }
    fn record(&mut self, _: &RecordDefinition, scope: Scope<'_>) -> CResult {
        self.storage(scope)
    }
    fn union(&mut self, _: &UnionDefinition, scope: Scope<'_>) -> CResult {
        self.storage(scope)
    }
    fn opaque(&mut self, _: &RecordSpec, scope: Scope<'_>) -> CResult {
        self.storage(scope)
    }
    fn bits(&mut self, bits: &BitsDefinition, scope: Scope<'_>) -> CResult {
        self.output
            .constants::<64>(&self.prefix, &[("SIZE", bits.scalar().width())])?;
        self.output
            .constants::<32>(&self.prefix, &[("BIT_WIDTH", bits.bit_width())])?;
        self.output
            .constants::<64>(&self.prefix, &[("DECLARED_MASK", bits.declared_mask())])?;
        emit_all(Node::Bits(bits).children(), scope, self)
    }
    fn bit_field(&mut self, field: &BitsFieldSpec, scope: Scope<'_>) -> CResult {
        let prefix = format!("{}_{}", self.prefix, naming::c_identifier(field.name()));
        self.output.constants::<32>(
            &prefix,
            &[("LSB", field.lsb()), ("BITS", field.bit_width())],
        )?;
        emit_all(Node::BitField(field).children(), scope, self)
    }
    fn enumeration(&mut self, enumeration: EnumDefinition, scope: Scope<'_>) -> CResult {
        let prefix = match scope.bit {
            Some(bit) => format!("{}_{}", self.prefix, naming::c_identifier(bit)),
            None => self.prefix.clone(),
        };
        self.enum_constants(&prefix, enumeration)
    }
    fn register_map(&mut self, _: MapView<'_>, scope: Scope<'_>) -> CResult {
        let map = scope
            .map_layout()
            .ok_or(CHeaderExportError::InvalidVisit("map layout"))?;
        if let Some(view) = &map.instance {
            self.map_names = register_map::map_names(view);
            self.output.documentation("", view.documentation)?;
            self.map_constants(view.extent)?;
            for (path, docs) in &view.groups {
                self.output.documentation(path, *docs)?;
            }
            emit_all(MapView::Resolved(view).children(), scope, self)?;
        }
        if let Some(program) = &map.program {
            self.map_layout(program)?;
        }
        Ok(())
    }
    fn placement(&mut self, value: PlacementView<'_>, scope: Scope<'_>) -> CResult {
        if let PlacementView::Bank {
            view,
            index,
            expanded,
        } = value
        {
            let name = self.bank_name(index)?;
            self.output.text.push('\n');
            if expanded {
                self.output.constants::<64>(&name, &[("INDEX", index)])?;
                if let Some((parent, instance)) = view.parent {
                    self.output.constants::<64>(
                        &name,
                        &[("PARENT_INDEX", parent), ("PARENT_INSTANCE", instance)],
                    )?;
                }
            } else {
                self.output.documentation("", view.documentation)?;
            }
            register_map::bank_metadata(&mut self.output, &name, view)?;
        }
        emit_all(value.children(), scope, self)
    }
    fn register(&mut self, register: &RegisterInfo, scope: Scope<'_>) -> CResult {
        let location = scope.register_location();
        match location {
            RegisterLocation::Root => {
                self.output.documentation("", register.documentation())?;
                self.map_constants(register.width())?;
            }
            RegisterLocation::Standalone(_) | RegisterLocation::Template { .. } => {
                if matches!(location, RegisterLocation::Standalone(_)) {
                    self.output.text.push('\n');
                }
                if let Some(PlacementView::Register { view, .. }) = scope.placement {
                    self.output.documentation("", view.documentation)?;
                }
            }
            RegisterLocation::Instance { .. } => {}
        }
        let offset = self.register_offset(scope)?;
        if matches!(location, RegisterLocation::Instance { .. }) {
            let (name, offset) =
                offset.ok_or(CHeaderExportError::InvalidVisit("instance offset"))?;
            return self.output.unsigned::<64>(&name, offset);
        }
        let name = self.register_name(location)?;
        for field in register.fields() {
            self.output
                .documentation(field.name(), field.documentation())?;
            if let Some(enumeration) = field.definition().enum_definition() {
                self.enum_constants(
                    &format!("{name}_{}", naming::c_identifier(field.name())),
                    enumeration,
                )?;
            }
        }
        let names = register_map::RegisterNames {
            prefix: &self.prefix,
            name: &name,
            offset: offset
                .as_ref()
                .map(|(name, offset)| (name.as_str(), *offset)),
        };
        register_map::push_register(&mut self.output, names, register)
    }
    fn finish(self) -> CResult<Vec<Artifact>> {
        Ok(self.artifacts)
    }
}
