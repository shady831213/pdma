use super::{Call, Expression, LayoutProgram, LayoutProperty, Member, Statement, Symbol};
use crate::ast::{ChildLayout, LayoutError, MapLayoutPlan};

impl MapLayoutPlan {
    /// Lowers checked formulas into ordered computation after optional parameter binding.
    ///
    /// # Errors
    /// Rejects invalid concrete layouts or unresolved expression operands.
    pub fn program(&self) -> Result<LayoutProgram, LayoutError> {
        if self.parameters().is_empty() {
            lower(&self.bind(&[])?)
        } else {
            lower(self)
        }
    }
}

fn lower(plan: &MapLayoutPlan) -> Result<LayoutProgram, LayoutError> {
    let symbolic = !plan.parameters().is_empty();
    let mut program = LayoutProgram {
        alignment: plan.alignment(),
        parameters: plan.parameters().to_vec(),
        members: vec![Member::Extent],
        locals: Vec::new(),
        body: Vec::new(),
    };
    if symbolic {
        program.locals.push(Symbol::Cursor);
        program.body.push(Statement::Assign {
            target: Symbol::Cursor,
            value: Expression::Literal(0),
        });
    }
    for placement in plan.placements() {
        let name = placement.reference.name();
        if let ChildLayout::Call { arguments, .. } = &placement.child {
            program.body.push(Statement::Call(Call {
                name,
                definition: placement.child_definition,
                definition_name: placement.child_name,
                arguments: arguments
                    .iter()
                    .map(|arg| Expression::lower(arg, None))
                    .collect::<Result<_, _>>()?,
            }));
        }
        for (property, expression) in [
            (LayoutProperty::Offset, &placement.offset),
            (LayoutProperty::Count, &placement.count),
            (LayoutProperty::Stride, &placement.stride),
        ] {
            program.members.push(Member::Placement { name, property });
            program.body.push(Statement::Assign {
                target: Symbol::Placement { name, property },
                value: Expression::lower(expression, Some(placement))?,
            });
        }
        if symbolic {
            program.body.push(Statement::Define {
                target: Symbol::End(name),
                value: Expression::lower(&placement.end, Some(placement))?,
            });
            program.body.push(Statement::Assign {
                target: Symbol::Cursor,
                value: Expression::lower(&placement.cursor, Some(placement))?,
            });
        }
    }
    program.body.push(Statement::Assign {
        target: Symbol::Extent,
        value: Expression::lower(plan.extent(), None)?,
    });
    Ok(program)
}
