use crate::abi::{AbiId, DmaAddress};

/// Source and destination IOVAs shared by copy operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyIovas {
    source: DmaAddress,
    destination: DmaAddress,
}

impl CopyIovas {
    /// Creates a pair of absolute IOVAs.
    pub const fn new(source: DmaAddress, destination: DmaAddress) -> Self {
        Self {
            source,
            destination,
        }
    }
}

/// ABI-v1 linear-copy command payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiCopy {
    command_id: AbiId,
    iovas: CopyIovas,
    length: u32,
}

impl AbiCopy {
    /// Creates a fixed-width linear-copy command.
    pub const fn new(command_id: AbiId, iovas: CopyIovas, length: u32) -> Self {
        Self {
            command_id,
            iovas,
            length,
        }
    }

    /// Command identifier.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }

    /// Absolute source IOVA.
    pub const fn source(self) -> DmaAddress {
        self.iovas.source
    }

    /// Absolute destination IOVA.
    pub const fn destination(self) -> DmaAddress {
        self.iovas.destination
    }

    /// Transfer length in bytes.
    pub const fn length(self) -> u32 {
        self.length
    }
}

/// Source and destination row strides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyStrides {
    source: u32,
    destination: u32,
}

impl CopyStrides {
    /// Creates a pair of fixed-width row strides.
    pub const fn new(source: u32, destination: u32) -> Self {
        Self {
            source,
            destination,
        }
    }
}

/// Fixed-width strided-copy dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StridedCopyShape {
    rows: u32,
    row_bytes: u32,
    strides: CopyStrides,
}

impl StridedCopyShape {
    /// Creates strided-copy dimensions.
    pub const fn new(rows: u32, row_bytes: u32, strides: CopyStrides) -> Self {
        Self {
            rows,
            row_bytes,
            strides,
        }
    }
}

/// ABI-v1 strided-copy command payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiStridedCopy {
    command_id: AbiId,
    iovas: CopyIovas,
    shape: StridedCopyShape,
}

impl AbiStridedCopy {
    /// Creates a fixed-width strided-copy command.
    pub const fn new(command_id: AbiId, iovas: CopyIovas, shape: StridedCopyShape) -> Self {
        Self {
            command_id,
            iovas,
            shape,
        }
    }

    /// Command identifier.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }

    /// Absolute source-start IOVA.
    pub const fn source(self) -> DmaAddress {
        self.iovas.source
    }

    /// Absolute destination-start IOVA.
    pub const fn destination(self) -> DmaAddress {
        self.iovas.destination
    }

    /// Number of rows.
    pub const fn rows(self) -> u32 {
        self.shape.rows
    }

    /// Bytes copied per row.
    pub const fn row_bytes(self) -> u32 {
        self.shape.row_bytes
    }

    /// Source distance between row starts.
    pub const fn source_stride(self) -> u32 {
        self.shape.strides.source
    }

    /// Destination distance between row starts.
    pub const fn destination_stride(self) -> u32 {
        self.shape.strides.destination
    }
}
