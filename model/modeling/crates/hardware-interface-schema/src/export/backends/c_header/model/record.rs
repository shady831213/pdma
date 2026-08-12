use crate::{Endian, ResolvedMemberKind, ResolvedRecord, ScalarKind};

/// One resolved scalar row consumed by C-header renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCHeaderField {
    id: u16,
    path: String,
    name: String,
    offset: usize,
    width: usize,
    scalar: ScalarKind,
    endian: Endian,
}

impl RecordCHeaderField {
    /// Returns the stable field identity.
    pub const fn id(&self) -> u16 {
        self.id
    }

    /// Returns the resolved root-qualified path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the deterministic C source name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the absolute byte offset.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the scalar byte width.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the scalar kind.
    pub const fn scalar(&self) -> ScalarKind {
        self.scalar
    }

    /// Returns the scalar byte order.
    pub const fn endian(&self) -> Endian {
        self.endian
    }
}

/// Complete ordered typed model for one resolved record C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCHeaderModel {
    size: usize,
    alignment: usize,
    fields: Vec<RecordCHeaderField>,
}

impl RecordCHeaderModel {
    /// Projects a resolved graph into a complete typed C fragment model.
    #[must_use]
    pub fn from_resolved(schema: &ResolvedRecord) -> Self {
        let fields = schema
            .members()
            .iter()
            .filter_map(|member| match member.kind() {
                ResolvedMemberKind::Field { id, definition } => Some(RecordCHeaderField {
                    id,
                    path: member.path().into(),
                    name: relative_path(schema.root(), member.path()).into(),
                    offset: member.absolute_offset(),
                    width: definition.width(),
                    scalar: definition.scalar(),
                    endian: definition.endian(),
                }),
                ResolvedMemberKind::Record { .. } => None,
            })
            .collect();
        Self {
            size: schema.size(),
            alignment: schema.alignment(),
            fields,
        }
    }

    /// Returns the record byte size.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns the record byte alignment.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }

    /// Returns scalar fields in resolved declaration order.
    pub fn fields(&self) -> &[RecordCHeaderField] {
        &self.fields
    }
}

fn relative_path<'a>(root: &str, path: &'a str) -> &'a str {
    if root.is_empty() {
        path.strip_prefix('.').unwrap_or(path)
    } else {
        path.strip_prefix(root)
            .and_then(|relative| relative.strip_prefix('.'))
            .unwrap_or(path)
    }
}
