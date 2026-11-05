use super::{BankView, RegisterMapView, RegisterView, ViewPathSegment};
use crate::{
    Documentation,
    ast::{DefinitionBody, LayoutError, MapDefinition},
};

impl RegisterMapView {
    pub(super) fn from_definition(
        name: &str,
        docs: Documentation,
        map: &MapDefinition,
        args: &[usize],
    ) -> Result<Self, LayoutError> {
        let mut walk = InstanceWalk {
            registers: Vec::new(),
            banks: Vec::new(),
            groups: Vec::new(),
        };
        let extent = walk.map(
            map,
            args,
            &Scope {
                base: 0,
                path: name.into(),
                segments: Vec::new(),
                parent: None,
            },
        )?;
        Ok(Self {
            extent,
            alignment: Some(map.alignment()),
            documentation: docs,
            registers: walk.registers,
            banks: walk.banks,
            groups: walk.groups,
            expanded_banks: true,
        })
    }
}
struct Scope {
    base: usize,
    path: String,
    segments: Vec<ViewPathSegment>,
    parent: Option<(usize, usize)>,
}
struct InstanceWalk {
    registers: Vec<RegisterView>,
    banks: Vec<BankView>,
    groups: Vec<(String, Documentation)>,
}
impl InstanceWalk {
    fn map(
        &mut self,
        map: &MapDefinition,
        args: &[usize],
        scope: &Scope,
    ) -> Result<usize, LayoutError> {
        let geometry = map.resolve(args)?;
        for (placement, shape) in map.placements().iter().zip(geometry.placements) {
            let path = format!("{}.{}", scope.path, placement.reference().name());
            let base = scope
                .base
                .checked_add(shape.offset)
                .ok_or(LayoutError::Geometry)?;
            let bank_index = if placement.is_bank() {
                let index = self.banks.len();
                self.banks.push(BankView {
                    path: path.clone(),
                    segments: scope
                        .segments
                        .iter()
                        .cloned()
                        .chain([ViewPathSegment {
                            name: shape.name.into(),
                            index: None,
                        }])
                        .collect(),
                    name: shape.name,
                    base: crate::ast::LayoutExpr::Constant(base),
                    count: crate::ast::LayoutExpr::Constant(shape.count),
                    stride: crate::ast::LayoutExpr::Constant(shape.stride),
                    parent: scope.parent,
                    documentation: placement.documentation(),
                    registers: Vec::new(),
                    instances: Vec::new(),
                });
                Some(index)
            } else {
                self.groups.push((path.clone(), placement.documentation()));
                None
            };
            for i in 0..shape.count {
                let offset = i
                    .checked_mul(shape.stride)
                    .and_then(|step| base.checked_add(step))
                    .ok_or(LayoutError::Geometry)?;
                let child_scope = Scope {
                    base: offset,
                    path: bank_index.map_or_else(|| path.clone(), |_| format!("{path}[{i}]")),
                    segments: scope
                        .segments
                        .iter()
                        .cloned()
                        .chain([ViewPathSegment {
                            name: shape.name.into(),
                            index: bank_index.map(|_| i),
                        }])
                        .collect(),
                    parent: bank_index.map(|bank| (bank, i)).or(scope.parent),
                };
                let first_register = self.registers.len();
                match placement.reference().target().body() {
                    DefinitionBody::Register(register) => self.registers.push(RegisterView {
                        path: child_scope.path,
                        segments: child_scope.segments,
                        name: shape.name,
                        offset,
                        documentation: placement
                            .documentation()
                            .with_fallback(register.documentation()),
                        register: register.clone(),
                    }),
                    DefinitionBody::RegisterMap(child) => {
                        self.map(child, &shape.arguments, &child_scope)?;
                    }
                    DefinitionBody::Opaque(_) => {}
                    DefinitionBody::Bits(_)
                    | DefinitionBody::Record(_)
                    | DefinitionBody::Union { .. } => return Err(LayoutError::Geometry),
                }
                if let Some(index) = bank_index {
                    let instance = self
                        .registers
                        .get(first_register..)
                        .ok_or(LayoutError::Geometry)?
                        .to_vec();
                    let bank = self.banks.get_mut(index).ok_or(LayoutError::Geometry)?;
                    if i == 0 {
                        bank.registers = instance
                            .iter()
                            .map(|register| {
                                let mut register = register.clone();
                                register.offset = register
                                    .offset
                                    .checked_sub(base)
                                    .ok_or(LayoutError::Geometry)?;
                                Ok(register)
                            })
                            .collect::<Result<Vec<_>, LayoutError>>()?;
                    }
                    bank.instances.push(instance);
                }
            }
        }
        Ok(geometry.extent)
    }
}
