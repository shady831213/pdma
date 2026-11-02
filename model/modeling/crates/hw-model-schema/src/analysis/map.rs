use super::{AnalyzedSchema, Layout, MapLayout, Unit};
use crate::{Documentation, ast::RegisterInfo, register_map::ResolvedRegisterMap};

/// One path component, retaining bank indices independently of target spelling.
#[derive(Debug, Clone)]
pub struct ViewPathSegment {
    /// Original placement name.
    pub name: String,
    /// Bank instance, including zero for a bank of one.
    pub index: Option<usize>,
}

/// One prepared register placement with effective access permissions.
#[derive(Debug, Clone)]
pub struct RegisterView {
    /// Root-qualified source path.
    pub path: String,
    /// Structural path excluding the root, retaining bank indices.
    pub segments: Vec<ViewPathSegment>,
    /// Original placement name.
    pub name: &'static str,
    /// Offset in the enclosing map or bank template.
    pub offset: usize,
    /// Effective placement documentation.
    pub documentation: Documentation,
    /// Effective register definition, including projected masks and permissions.
    pub register: RegisterInfo,
}

/// Prepared bank topology; empty projected banks remain present.
#[derive(Debug, Clone)]
pub struct BankView {
    /// Source path.
    pub path: String,
    /// Placement name.
    pub name: &'static str,
    /// First instance offset.
    pub base: usize,
    /// Number of instances.
    pub count: usize,
    /// Instance spacing.
    pub stride: usize,
    /// Enclosing bank position and instance, when nested.
    pub parent: Option<(usize, usize)>,
    /// Placement documentation.
    pub documentation: Documentation,
    /// Template members in declaration order.
    pub registers: Vec<RegisterView>,
    /// Expanded members grouped by instance, already in emission order.
    pub instances: Vec<Vec<RegisterView>>,
}

/// Concrete map metadata prepared once for any target language.
#[derive(Debug, Clone)]
pub struct RegisterMapView {
    /// Full occupied extent.
    pub extent: usize,
    /// Root alignment when retained by the source definition.
    pub alignment: Option<usize>,
    /// Map documentation.
    pub documentation: Documentation,
    /// Nested group paths and placement documentation.
    pub groups: Vec<(String, Documentation)>,
    /// Standalone declarations, or all expanded leaves for ordered maps.
    pub registers: Vec<RegisterView>,
    /// Bank topology and template declarations.
    pub banks: Vec<BankView>,
    /// Whether the source declares complete nested blocks rather than leaf templates.
    pub expanded_banks: bool,
}

impl AnalyzedSchema {
    /// Uses an explicit permission view as authoritative generation input.
    pub fn register_map<I: Copy + Eq, F: Copy + Eq + core::fmt::Debug>(
        name: &str,
        map: &ResolvedRegisterMap<I, F>,
    ) -> Result<Self, super::AnalysisError> {
        Self::finish(vec![Unit {
            declaration: None,
            id: name.into(),
            name: name.into(),
            documentation: map.documentation(),
            references: Vec::new(),
            body: Layout::Map(MapLayout {
                plan: None,
                arguments: None,
                instance: Some(RegisterMapView::from_resolved(map)),
            }),
        }])
    }
}

impl RegisterMapView {
    fn from_resolved<I: Copy + Eq, F: Copy + Eq + core::fmt::Debug>(
        map: &ResolvedRegisterMap<I, F>,
    ) -> Self {
        let expanded_banks = map.banks().iter().any(|bank| bank.ordered);
        let registers = map
            .registers()
            .iter()
            .filter(|register| {
                expanded_banks
                    || register.instance().is_none()
                    || !map.banks().iter().any(|bank| {
                        bank.expanded_registers()
                            .iter()
                            .any(|member| member.path() == register.path())
                    })
            })
            .map(|register| register_view(register, map.root()))
            .collect();
        let banks = map
            .banks()
            .iter()
            .map(|bank| BankView {
                path: bank.path().into(),
                name: bank.name(),
                base: bank.base(),
                count: bank.count(),
                stride: bank.stride(),
                parent: map
                    .enclosing_banks(bank.path())
                    .next_back()
                    .map(|(index, _, instance)| (index, instance)),
                documentation: bank.documentation(),
                registers: bank
                    .registers()
                    .iter()
                    .map(|register| RegisterView {
                        path: register.name().into(),
                        segments: vec![ViewPathSegment {
                            name: register.name().into(),
                            index: None,
                        }],
                        name: register.name(),
                        offset: register.offset(),
                        documentation: register.documentation(),
                        register: RegisterInfo::from_spec(register.definition()),
                    })
                    .collect(),
                instances: (0..bank.count())
                    .map(|index| {
                        bank.expanded_registers()
                            .iter()
                            .filter(|register| {
                                if bank.ordered {
                                    register.bank_ancestry().iter().any(|ancestor| {
                                        ancestor.path() == bank.path() && ancestor.index() == index
                                    })
                                } else {
                                    register.instance() == Some(index)
                                }
                            })
                            .map(|register| register_view(register, map.root()))
                            .collect()
                    })
                    .collect(),
            })
            .collect();
        Self {
            extent: map.size(),
            alignment: None,
            documentation: map.documentation(),
            registers,
            banks,
            expanded_banks,
            groups: map
                .placements()
                .iter()
                .filter(|node| !node.placement().is_bank())
                .map(|node| (node.path().into(), node.documentation()))
                .collect(),
        }
    }
}

fn register_view<I: Copy, F: Copy + Eq + core::fmt::Debug>(
    register: &crate::register_map::ResolvedRegister<I, F>,
    root: &str,
) -> RegisterView {
    RegisterView {
        path: register.path().into(),
        segments: register
            .path()
            .strip_prefix(root)
            .unwrap_or_else(|| register.path())
            .trim_start_matches('.')
            .split('.')
            .map(
                |part| match part.strip_suffix(']').and_then(|s| s.rsplit_once('[')) {
                    Some((name, index)) if index.parse::<usize>().is_ok() => ViewPathSegment {
                        name: name.into(),
                        index: index.parse().ok(),
                    },
                    _ => ViewPathSegment {
                        name: part.into(),
                        index: None,
                    },
                },
            )
            .collect(),
        name: register.name(),
        offset: register.offset(),
        documentation: register.documentation(),
        register: RegisterInfo::from_spec(register.definition()),
    }
}
