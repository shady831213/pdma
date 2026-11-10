use his::{
    analysis::{AnalyzedSchema, Pass},
    ast::{DefinitionId, record::Node},
    export::{
        Visit,
        record::{self, Context},
        value::{self, Emit as _},
        view,
    },
    traversal::{self, NodeId},
};
use std::{
    collections::HashMap,
    io::{self, Write},
};

/// Application-owned record report renderer consuming a target-specific analysis pass.
#[derive(Debug)]
pub struct TelemetryRecordRenderer {
    selected: NodeId,
    offsets: HashMap<NodeId, usize>,
    first: bool,
}
impl TelemetryRecordRenderer {
    /// Prepares absolute offsets without exposing semantic traversal to render callbacks.
    pub fn new(schema: &AnalyzedSchema) -> io::Result<Self> {
        Ok(Self {
            selected: schema
                .definition(DefinitionId::of::<crate::DownPair>())
                .ok_or_else(|| io::Error::other("selected record"))?
                .id
                .clone(),
            offsets: ByteOffsets.run(schema)?,
            first: true,
        })
    }
}
impl record::Backend for TelemetryRecordRenderer {
    fn entry(&mut self, ctx: Context<'_>, w: &mut dyn Write) -> io::Result<()> {
        if ctx.entry_id() == &self.selected {
            self.first = true;
            self.emit(ctx.current(), w)?;
        }
        Ok(())
    }
    fn record<'a>(
        &mut self,
        node: view::Record<'a>,
        ctx: Context<'a>,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        if ctx.is_entry() {
            write!(w, "record_size={};record=", node.spec().size())?;
        }
        self.emit_all(ctx.children(), w)?;
        if ctx.is_entry() {
            w.write_all(b";")?;
        }
        Ok(())
    }
    fn field<'a>(
        &mut self,
        field: view::Field<'a>,
        ctx: Context<'a>,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let offset = self
            .offsets
            .get(ctx.id())
            .ok_or_else(|| io::Error::other("field offset"))?;
        if !self.first {
            w.write_all(b",")?;
        }
        self.first = false;
        write!(
            w,
            "{}.{}@{}+{}",
            ctx.entry_name(),
            ctx.path(),
            offset,
            field.width()
        )?;
        self.emit_all(ctx.children(), w)
    }
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        target: Visit<Context<'a>>,
        _: Context<'a>,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        self.emit(target, w)
    }
}
impl value::Backend for TelemetryRecordRenderer {
    type Domain = record::Domain;
    type Error = io::Error;
    type Context<'a> = Context<'a>;
}
struct ByteOffsets;
impl Pass<&AnalyzedSchema> for ByteOffsets {
    type Output = HashMap<NodeId, usize>;
    type Error = io::Error;
    fn run(self, schema: &AnalyzedSchema) -> io::Result<Self::Output> {
        let mut offsets = HashMap::new();
        for entry in traversal::record::definitions(schema) {
            collect(entry.context(), &mut offsets)?;
        }
        Ok(offsets)
    }
}
fn collect(
    ctx: &traversal::record::Context<'_>,
    offsets: &mut HashMap<NodeId, usize>,
) -> io::Result<()> {
    if matches!(ctx.node(), Node::Field(_)) {
        let offset = ctx
            .ancestors()
            .iter()
            .copied()
            .chain([ctx.node()])
            .try_fold(0_usize, |base, node| match node {
                Node::Field(field) => base.checked_add(field.offset()),
                _ => Some(base),
            })
            .ok_or_else(|| io::Error::other("offset overflow"))?;
        offsets.insert(ctx.id().clone(), offset);
    }
    if let Some(target) = ctx.referent() {
        collect(target.context(), offsets)?;
    }
    for child in ctx.children() {
        collect(child.context(), offsets)?;
    }
    Ok(())
}
