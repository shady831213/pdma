use super::{CompleteMapLayout, Constraint};
use crate::ast::{LayoutError, LayoutExpr, MapGeometry, PlacementGeometry};
impl CompleteMapLayout {
    /// Evaluates complete formulas and all retained nested geometry constraints.
    pub fn evaluate(&self, arguments: &[usize]) -> Result<MapGeometry, LayoutError> {
        let eval = |e: &crate::ast::LayoutExpr| e.evaluate(self.parameters, arguments);
        for constraint in &self.constraints {
            constraint.check(|expression| eval(expression).map(Some))?;
        }
        let placements = self
            .placements
            .iter()
            .map(|p| {
                Ok(PlacementGeometry {
                    name: p.source.name(),
                    definition: p.source.target().id(),
                    arguments: p.arguments.iter().map(eval).collect::<Result<_, _>>()?,
                    offset: eval(&p.offset)?,
                    count: eval(&p.count)?,
                    stride: eval(&p.stride)?,
                    extent: eval(&p.child_extent)?,
                })
            })
            .collect::<Result<_, LayoutError>>()?;
        Ok(MapGeometry {
            alignment: self.alignment,
            extent: eval(&self.extent)?,
            placements,
        })
    }
}

impl Constraint {
    pub(in crate::analysis) fn check(
        &self,
        eval: impl Fn(&LayoutExpr) -> Result<Option<usize>, LayoutError>,
    ) -> Result<(), LayoutError> {
        let valid = match self {
            Self::Positive(e) => eval(e)?.map(|value| value != 0),
            Self::Aligned(e, a) => eval(e)?.map(|value| value.is_multiple_of(*a)),
            Self::Fits(a, b) => eval(a)?
                .zip(eval(b)?)
                .map(|(occupied, extent)| occupied <= extent),
            Self::Disjoint(a, b, c, d) => eval(a)?.zip(eval(b)?).zip(eval(c)?.zip(eval(d)?)).map(
                |((start, end), (other_start, other_end))| start >= other_end || end <= other_start,
            ),
        };
        if valid == Some(false) {
            return Err(match self {
                Self::Disjoint(..) => LayoutError::Overlap,
                Self::Positive(_) | Self::Aligned(..) | Self::Fits(..) => LayoutError::Geometry,
            });
        }
        Ok(())
    }
}
