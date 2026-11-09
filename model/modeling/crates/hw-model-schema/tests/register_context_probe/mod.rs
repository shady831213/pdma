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
    fn reference<'a>(
        &mut self,
        _: hw_model_schema::export::view::Reference<'a>,
        target: hw_model_schema::export::view::Target<'a>,
        _: hw_model_schema::export::Visit<RegisterContext<'a>>,
        ctx: RegisterContext<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(target.name, "Word");
        assert!(ctx.placement().is_some());
        Ok(())
    }
    fn map_layout<'a>(
        &mut self,
        _: hw_model_schema::export::view::MapLayout<'a>,
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
        match ctx.parent_kind().ok_or("missing parent")? {
            hw_model_schema::export::Kind::MapLayout => {
                assert_eq!(property.owner(), None);
            }
            hw_model_schema::export::Kind::Placement => {
                assert_eq!(property.owner(), ctx.parent_name().as_deref());
            }
            hw_model_schema::export::Kind::Bank => {
                use hw_model_schema::ast::register::LayoutPropertyKind as K;
                let bank = ctx.bank().ok_or("missing bank")?;
                assert_eq!(
                    (bank.index(), bank.name(), bank.path()),
                    (0, "zeta", "Pair.zeta")
                );
                assert_eq!(property.owner(), Some(bank.name()));
                let (name, expected) = match property.kind() {
                    K::Offset => ("offset", 0),
                    K::Count => ("count", self.binding.ok_or("bound bank count")?),
                    K::Stride => ("stride", 4),
                    K::End | K::OccupiedEnd | K::Extent => return Err("unexpected bank property"),
                };
                assert_eq!(property.name(), name);
                self.push(format!("bank:{}:{name}={expected}", bank.name()), writer)?;
            }
            _ => return Err("wrong parent"),
        }
        self.push(
            format!("{}:{}", property.owner().unwrap_or("root"), property.name()),
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
    fn expression<'a>(
        &mut self,
        expression: hw_model_schema::export::view::Expression<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if ctx.bank().is_some() {
            use hw_model_schema::ast::register::LayoutPropertyKind as K;
            let property = ctx.layout_property().ok_or("bank expression property")?;
            let expected = match property.kind() {
                K::Offset => 0,
                K::Count => self.binding.ok_or("bound bank count")?,
                K::Stride => 4,
                K::End | K::OccupiedEnd | K::Extent => return Err("unexpected bank property"),
            };
            assert_eq!(expression.evaluate(&[], &[]), Ok(expected));
        }
        self.emit_all(ctx.children(), writer)
    }
}
impl hw_model_schema::export::OutputBackend for Probe {
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.root_name().to_owned(),
            media_type: "text/plain".into(),
        })
    }
}
