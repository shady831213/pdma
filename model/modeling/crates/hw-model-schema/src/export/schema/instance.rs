use super::{BankView, ExportPathSegment, RegisterMapView, RegisterView};
use crate::{
    Documentation,
    graph::{DefinitionBody, LayoutError, MapDefinition},
};

impl RegisterMapView {
    pub(super) fn from_definition(
        name: &str,
        docs: Documentation,
        map: &MapDefinition,
        args: &[usize],
    ) -> Result<Self, LayoutError> {
        let registers = map
            .registers(args)?
            .into_iter()
            .map(|leaf| {
                let segments: Vec<_> = leaf
                    .path()
                    .iter()
                    .map(|p| ExportPathSegment {
                        name: p.name.into(),
                        index: p.index,
                    })
                    .collect();
                let relative = segments
                    .iter()
                    .map(|p| {
                        p.index
                            .map_or_else(|| p.name.clone(), |i| format!("{}[{i}]", p.name))
                    })
                    .collect::<Vec<_>>()
                    .join(".");
                let last = leaf.path().last().ok_or(LayoutError::Geometry)?;
                Ok(RegisterView {
                    path: format!("{name}.{relative}"),
                    segments,
                    name: last.name,
                    offset: leaf.offset(),
                    documentation: last
                        .documentation
                        .with_fallback(leaf.register().documentation()),
                    register: leaf.register().clone(),
                })
            })
            .collect::<Result<Vec<_>, LayoutError>>()?;
        let mut walk = InstanceWalk {
            registers,
            banks: Vec::new(),
            groups: Vec::new(),
        };
        walk.map(
            map,
            args,
            &Scope {
                base: 0,
                path: name.into(),
                parent: None,
            },
        )?;
        Ok(Self {
            extent: map.resolve(args)?.extent,
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
    ) -> Result<(), LayoutError> {
        let geometry = map.resolve(args)?;
        for (placement, shape) in map.placements().iter().zip(geometry.placements) {
            let path = format!("{}.{}", scope.path, placement.reference().name());
            let base = scope
                .base
                .checked_add(shape.offset)
                .ok_or(LayoutError::Geometry)?;
            let bank_index = if placement.is_bank() {
                let index = self.banks.len();
                let instances = (0..shape.count)
                    .map(|i| {
                        let prefix = format!("{path}[{i}]");
                        self.registers
                            .iter()
                            .filter(|r| {
                                r.path == prefix
                                    || r.path
                                        .strip_prefix(&prefix)
                                        .is_some_and(|tail| tail.starts_with('.'))
                            })
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let registers = instances
                    .first()
                    .into_iter()
                    .flatten()
                    .map(|r| {
                        let mut r = r.clone();
                        r.offset = r.offset.checked_sub(base).ok_or(LayoutError::Geometry)?;
                        Ok(r)
                    })
                    .collect::<Result<Vec<_>, LayoutError>>()?;
                self.banks.push(BankView {
                    path: path.clone(),
                    name: shape.name,
                    base,
                    count: shape.count,
                    stride: shape.stride,
                    parent: scope.parent,
                    documentation: placement.documentation(),
                    registers,
                    instances,
                });
                Some(index)
            } else {
                self.groups.push((path.clone(), placement.documentation()));
                None
            };
            if let DefinitionBody::RegisterMap(child) = placement.reference().target().body() {
                for i in 0..shape.count {
                    let offset = i
                        .checked_mul(shape.stride)
                        .and_then(|step| base.checked_add(step))
                        .ok_or(LayoutError::Geometry)?;
                    self.map(
                        child,
                        &shape.arguments,
                        &Scope {
                            base: offset,
                            path: bank_index
                                .map_or_else(|| path.clone(), |_| format!("{path}[{i}]")),
                            parent: bank_index.map(|bank| (bank, i)).or(scope.parent),
                        },
                    )?;
                }
            }
        }
        Ok(())
    }
}
