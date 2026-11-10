use crate::export::backends::sv::{Names, Symbols};
use crate::{export::EntrySymbols, traversal::NodeId};
use std::{
    fmt::Display,
    io::{self, Write},
};
#[derive(Debug, Default)]
pub(in crate::export::backends) struct Output {
    pub(in crate::export::backends) names: Names,
    pub(in crate::export::backends) package: Package,
}
impl Output {
    pub(in crate::export::backends) fn check_entry(
        &self,
        id: &NodeId,
        name: &str,
        symbols: &mut EntrySymbols,
    ) -> io::Result<()> {
        self.names.check(id, &format!("{name}Layout"), symbols)
    }
    pub(in crate::export::backends) fn begin(
        &mut self,
        id: &NodeId,
        name: &str,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        self.package = Package {
            owner: Some(id.clone()),
            symbols: Symbols::default(),
        };
        writeln!(
            w,
            "// Generated schema layout; do not edit.\npackage {};",
            self.names.get(id, &format!("{name}Layout"))
        )
    }
}
#[derive(Debug, Default)]
pub(in crate::export::backends) struct Package {
    owner: Option<NodeId>,
    symbols: Symbols,
}
impl Package {
    pub(in crate::export::backends) fn declare(&mut self, name: &str) -> io::Result<()> {
        self.symbols.declare(
            name,
            self.owner
                .as_ref()
                .ok_or_else(|| io::Error::other("no active SV package"))?,
        )
    }
    pub(in crate::export::backends) fn constant(
        &mut self,
        name: &str,
        ty: &str,
        value: impl Display,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        self.declare(name)?;
        writeln!(w, "    localparam {ty} {name} = {value};")
    }
    pub(in crate::export::backends) fn extent(
        &mut self,
        size: usize,
        align: usize,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        self.constant("EXTENT", "logic [63:0]", format_args!("64'd{size}"), w)?;
        self.constant("ALIGNMENT", "logic [63:0]", format_args!("64'd{align}"), w)
    }
}
