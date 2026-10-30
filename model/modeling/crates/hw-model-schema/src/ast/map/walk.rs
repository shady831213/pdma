use super::{LayoutError, MapDefinition};
use crate::{
    Documentation,
    ast::{DefinitionBody, DefinitionId, RegisterInfo},
};

/// One structural placement; a bank of one still has `Some(0)` as its index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPathSegment {
    /// Placement name, before target-language escaping or joining.
    pub name: &'static str,
    /// Instance index for a bank, absent for a scalar placement.
    pub index: Option<usize>,
    /// Placement documentation, independent of the referenced definition.
    pub documentation: Documentation,
}

/// One register leaf in a configured map, in depth-first declaration order.
#[derive(Debug, Clone)]
pub struct RegisterInstance {
    path: Vec<MapPathSegment>,
    offset: usize,
    definition: DefinitionId,
    register: RegisterInfo,
}
impl RegisterInstance {
    /// Structural path relative to the walked root, retaining bank indices.
    pub fn path(&self) -> &[MapPathSegment] {
        &self.path
    }
    /// Byte offset relative to the walked root.
    pub const fn offset(&self) -> usize {
        self.offset
    }
    /// Identity of the register definition, shared by repeated instances.
    pub const fn definition(&self) -> DefinitionId {
        self.definition
    }
    /// Register semantics and field metadata.
    pub const fn register(&self) -> &RegisterInfo {
        &self.register
    }
}

impl MapDefinition {
    /// Resolves and expands register leaves without choosing target names.
    ///
    /// # Errors
    /// Returns a layout error for invalid geometry or a child without register
    /// semantics (including opaque layouts); no partial result is returned.
    pub fn registers(&self, arguments: &[usize]) -> Result<Vec<RegisterInstance>, LayoutError> {
        let mut walk = RegisterWalk {
            path: Vec::new(),
            registers: Vec::new(),
        };
        walk.map(self, arguments, 0)?;
        Ok(walk.registers)
    }
}

struct RegisterWalk {
    path: Vec<MapPathSegment>,
    registers: Vec<RegisterInstance>,
}
impl RegisterWalk {
    fn map(
        &mut self,
        map: &MapDefinition,
        arguments: &[usize],
        base: usize,
    ) -> Result<(), LayoutError> {
        let geometry = map.resolve(arguments)?;
        for (placement, resolved) in map.placements().iter().zip(&geometry.placements) {
            let child = placement.reference().target();
            for index in 0..resolved.count {
                let offset = index
                    .checked_mul(resolved.stride)
                    .and_then(|step| step.checked_add(resolved.offset))
                    .and_then(|offset| offset.checked_add(base))
                    .ok_or(LayoutError::Geometry)?;
                self.path.push(MapPathSegment {
                    name: resolved.name,
                    index: placement.is_bank().then_some(index),
                    documentation: placement.documentation(),
                });
                match child.body() {
                    DefinitionBody::Register(register) => self.registers.push(RegisterInstance {
                        path: self.path.clone(),
                        offset,
                        definition: child.id(),
                        register: register.clone(),
                    }),
                    DefinitionBody::RegisterMap(child_map) => {
                        self.map(child_map, &resolved.arguments, offset)?;
                    }
                    DefinitionBody::Bits(_)
                    | DefinitionBody::Record(_)
                    | DefinitionBody::Union { .. }
                    | DefinitionBody::Opaque(_) => return Err(LayoutError::Geometry),
                }
                self.path.pop();
            }
        }
        Ok(())
    }
}
