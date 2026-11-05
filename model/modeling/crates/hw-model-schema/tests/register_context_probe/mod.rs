use super::*;
/// Trace backend with optional rejection and parameter binding checks.
#[derive(Debug)]
pub struct Probe {
    pub(super) events: Vec<String>,
    pub(super) fail: bool,
    pub(super) binding: Option<usize>,
}
impl Probe {
    fn push(&mut self, event: String, writer: &mut dyn std::io::Write) -> Result<(), &'static str> {
        if !self.events.is_empty() {
            writer.write_all(b"\n").map_err(|_| "writer rejected")?;
        }
        writer
            .write_all(event.as_bytes())
            .map_err(|_| "writer rejected")?;
        self.events.push(event);
        Ok(())
    }
}
impl register::Backend for Probe {
    fn map_layout<'a>(
        &mut self,
        _: &'a CompleteMapLayout,
        ctx: RegisterContext<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.declared_parameters(), &["n"]);
        assert_eq!(
            ctx.parameters(),
            if self.binding.is_some() {
                &[][..]
            } else {
                &["n"][..]
            }
        );
        assert_eq!(
            ctx.bindings().and_then(|b| b.first().copied()),
            self.binding
        );
        let mut children: Vec<_> = ctx.children().collect();
        children.reverse();
        self.emit_all(children, writer)
    }
    fn parameter(
        &mut self,
        p: Parameter,
        ctx: RegisterContext<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.declared_parameters().get(p.index), Some(&p.name));
        assert_eq!(
            p.binding,
            ctx.bindings().and_then(|b| b.get(p.index).copied())
        );
        self.push("parameter".into(), writer)?;
        Ok(())
    }
    fn layout_property<'a>(
        &mut self,
        property: LayoutProperty<'a>,
        ctx: RegisterContext<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.declared_parameters(), &["n"]);
        assert_eq!(
            ctx.parameters(),
            if self.binding.is_some() {
                &[][..]
            } else {
                &["n"][..]
            }
        );
        assert_eq!(
            ctx.bindings().and_then(|b| b.first().copied()),
            self.binding
        );
        match ctx.ancestors().last().ok_or("missing parent")? {
            hw_model_schema::traversal::register::Node::MapLayout(_) => {
                assert_eq!(property.owner, None);
            }
            hw_model_schema::traversal::register::Node::Placement(p) => {
                assert_eq!(property.owner, Some(p.reference.name()));
                assert_eq!(
                    ctx.resolve(&p.reference).ok_or("missing reference")?.name,
                    "Word"
                );
            }
            hw_model_schema::traversal::register::Node::Bank(bank) => {
                use hw_model_schema::ast::{LayoutExpr, register::LayoutPropertyKind as K};
                assert_eq!(
                    (bank.index(), bank.name(), bank.path()),
                    (0, "zeta", "Pair.zeta")
                );
                assert_eq!(ctx.bank().ok_or("missing bank")?.index(), bank.index());
                assert_eq!(property.owner, Some(bank.name()));
                let (name, expected) = match property.kind {
                    K::Offset => ("offset", 0),
                    K::Count => ("count", self.binding.ok_or("bound bank count")?),
                    K::Stride => ("stride", 4),
                    K::End | K::OccupiedEnd | K::Extent => return Err("unexpected bank property"),
                };
                assert_eq!(property.name, name);
                assert_eq!(property.expression, &LayoutExpr::Constant(expected));
                self.push(format!("bank:{}:{name}={expected}", bank.name()), writer)?;
            }
            _ => return Err("wrong parent"),
        }
        self.push(
            format!("{}:{}", property.owner.unwrap_or("root"), property.name),
            writer,
        )?;
        if self.fail {
            return Err("layout rejected");
        }
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for Probe {
    type Domain = register::Domain;
    type Error = &'static str;
    type Context<'a> = RegisterContext<'a>;
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
    }
}
