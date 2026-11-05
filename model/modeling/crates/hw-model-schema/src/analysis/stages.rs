//! Required semantic stages share the extension pass protocol.
use super::{AnalysisError, AnalyzedSchema, Bindings, Pass, Unit};
use crate::ast::{SchemaGraph, SchemaSet};

/// Resolves selected source definitions and their dependency identities.
#[derive(Debug, Clone, Copy)]
pub struct Resolve;
impl Pass<SchemaSet> for Resolve {
    type Output = SchemaGraph;
    type Error = AnalysisError;
    fn run(self, input: SchemaSet) -> Result<Self::Output, Self::Error> {
        super::resolve::run(input)
    }
}

/// Validates source semantics and the requested instance bindings.
#[derive(Debug, Clone, Copy)]
pub struct Validate<'a> {
    bindings: &'a Bindings,
}
impl<'a> Validate<'a> {
    /// Chooses the instance bindings checked by this validation stage.
    pub const fn new(bindings: &'a Bindings) -> Self {
        Self { bindings }
    }
}
impl<'a> Pass<&'a SchemaGraph> for Validate<'a> {
    type Output = ValidatedSchema<'a>;
    type Error = AnalysisError;
    fn run(self, graph: &'a SchemaGraph) -> Result<Self::Output, Self::Error> {
        super::resolve::bindings(graph, self.bindings)?;
        super::validate::run(graph)?;
        Ok(ValidatedSchema {
            graph,
            bindings: self.bindings,
        })
    }
}

/// Immutable source with successfully checked semantics and bindings.
///
/// Transformations return source input and must run validation again. There is no
/// mutable access to a validated source or constructor that skips validation.
/// ```compile_fail
/// use hw_model_schema::{analysis::{LayoutSchema, Pass}, ast::SchemaGraph};
/// fn skip_validation(graph: &SchemaGraph) {
///     LayoutSchema.run(graph);
/// }
/// ```
#[derive(Debug)]
pub struct ValidatedSchema<'a> {
    graph: &'a SchemaGraph,
    bindings: &'a Bindings,
}
impl<'a> ValidatedSchema<'a> {
    /// Borrows the checked definition graph for custom analysis.
    pub const fn graph(&self) -> &'a SchemaGraph {
        self.graph
    }
    /// Borrows the checked instance bindings.
    pub const fn bindings(&self) -> &'a Bindings {
        self.bindings
    }
    /// Discards the validation state before a source transformation.
    pub const fn into_source(self) -> &'a SchemaGraph {
        self.graph
    }
}

/// Computes target-independent layouts from validated source.
#[derive(Debug, Clone, Copy)]
pub struct LayoutSchema;
impl<'a> Pass<ValidatedSchema<'a>> for LayoutSchema {
    type Output = LaidOutSchema<'a>;
    type Error = AnalysisError;
    fn run(self, source: ValidatedSchema<'a>) -> Result<Self::Output, Self::Error> {
        let units = super::layout::run(source.graph, source.bindings)?;
        for unit in &units {
            super::validate::node(unit.node()?)?;
        }
        Ok(LaidOutSchema { source, units })
    }
}

/// Checked geometry before mathematical simplification.
#[derive(Debug)]
pub struct LaidOutSchema<'a> {
    source: ValidatedSchema<'a>,
    units: Vec<Unit>,
}
impl<'a> LaidOutSchema<'a> {
    /// Borrows completed geometry for a custom analysis before simplification.
    pub fn units(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter()
    }
    pub(super) fn into_units(self) -> Vec<Unit> {
        self.units
    }
    /// Borrows the validated source used to derive this geometry.
    pub const fn source(&self) -> &ValidatedSchema<'a> {
        &self.source
    }
    /// Discards all derived geometry and validation before transforming source.
    ///
    /// The returned input cannot be sent directly to simplification:
    /// ```compile_fail
    /// use hw_model_schema::analysis::{LaidOutSchema, Pass, Simplify};
    /// fn transform(layout: LaidOutSchema<'_>) {
    ///     Simplify.run(layout.into_source());
    /// }
    /// ```
    pub fn into_source(self) -> &'a SchemaGraph {
        self.source.into_source()
    }
}

/// Simplifies checked geometry and creates generation-ready input.
#[derive(Debug, Clone, Copy)]
pub struct Simplify;
impl Pass<LaidOutSchema<'_>> for Simplify {
    type Output = AnalyzedSchema;
    type Error = AnalysisError;
    fn run(self, input: LaidOutSchema<'_>) -> Result<Self::Output, Self::Error> {
        AnalyzedSchema::simplify_units(input.units)
    }
}
