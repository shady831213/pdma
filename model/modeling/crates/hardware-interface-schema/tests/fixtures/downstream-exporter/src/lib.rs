//! A standalone downstream consumer of the schema facade.

use core::convert::Infallible;

use his::{
    export::{Artifact, ArtifactSink, Exporter},
    field_schema, record_schema, register_field_schema, register_map,
    register_map::RegisterLocation,
    register_schema,
};

field_schema! { pub field DownLeU32 { scalar: u32, endian: Little } }
record_schema! { pub record DownHeader {
    size: 8, alignment: 4,
    fields: {
        sequence: DownLeU32 { id: 1, offset: 0 },
        flags: u16 { id: 2, offset: 4, endian: Little },
    }, records: {},
} }
record_schema! { pub record DownPair {
    size: 16, alignment: 8, fields: {},
    records: {
        left: DownHeader { id: 1, offset: 0 },
        right: DownHeader { id: 2, offset: 8 },
    },
} }
register_field_schema! { pub register_field DownEnable { bit_width: 1 } }
register_schema! { pub register DownControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: { enable: DownEnable { id: 1, lsb: 0 }, },
} }
register_schema! { pub register DownStatus {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff,
    fields: {},
} }
register_map! { pub map DownMap {
    size: 32,
    fixed: { control: DownControl { id: 1, name: "control", offset: 0 }, },
    banks: { lane: { count: 1, base: 16, stride: 16, members: {
        status: DownStatus { id: 2, name: "status", offset: 0 },
    } }, },
} }

record_schema! {
    pub record TelemetryRecord {
        size: 12,
        alignment: 4,
        fields: {
            sequence: u32 { id: 1, offset: 0, endian: Little },
            temperature: u16 { id: 2, offset: 4, endian: Big },
            flags: u8 { id: 3, offset: 6, endian: Little },
            samples: [u16; 2] { ids: [4, 5], offset: 8, stride: 2, endian: Little },
        }
    }
}

register_schema! {
    pub registers TelemetryRegisters {
        heartbeat: { id: 1, name: "heartbeat", offset: 0, width: 4, access: ReadOnly,
            reset: Zero(0), writable_mask: 0 },
        mode: { id: 2, name: "mode", offset: 4, width: 4, access: ReadWrite,
            reset: Constant(1), writable_mask: 0xffff_ffff },
    }
}

register_map! {
    pub map TelemetryMap {
        size: 128,
        fixed: {
            controller: { id: 1, name: "controller", offset: 4, width: 4, access: ReadWrite,
                reset: Constant(0), writable_mask: 0xffff_ffff },
            status: { id: 2, name: "status", offset: 12, width: 4, access: ReadOnly,
                reset: Zero(0), writable_mask: 0 },
        },
        banks: {
            lane: { count: 2, base: 32, stride: 32, members: {
                publish: { id: 3, name: "publish", offset: 0, width: 4, access: ReadWrite,
                    reset: Zero(0), writable_mask: 0xffff_ffff },
                fault: { id: 4, name: "fault", offset: 8, width: 4, access: ReadOnly,
                    reset: Zero(0), writable_mask: 0 },
            } },
        }
    }
}

/// Application-owned artifact collector with no filesystem policy.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TelemetrySink {
    artifacts: Vec<Artifact>,
}

impl TelemetrySink {
    /// Returns the artifacts in their downstream-defined emission order.
    #[must_use]
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }
}

impl ArtifactSink for TelemetrySink {
    type Error = Infallible;

    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error> {
        self.artifacts.push(artifact);
        Ok(())
    }
}

/// Application-owned exporter that renders the telemetry register-map summary.
#[derive(Debug, Default)]
pub struct TelemetryExporter;

/// Application-owned pair of already resolved schema graphs.
#[derive(Debug)]
pub struct TelemetryResolved {
    record: his::ResolvedRecord,
    map: his::ResolvedRegisterMap<TelemetryMapRegisterId>,
}

/// Exports fresh resolved graphs through an application-owned sink.
pub fn export_resolved(sink: &mut TelemetrySink) -> Option<()> {
    let resolved = TelemetryResolved {
        record: DownPair::SPEC.resolve().ok()?,
        map: TelemetryMap::SPEC.resolve().ok()?,
    };
    TelemetryExporter.export(&resolved, sink).ok()
}

impl Exporter<TelemetryResolved> for TelemetryExporter {
    type Error = Infallible;

    fn export<Sink>(
        &self,
        schema: &TelemetryResolved,
        sink: &mut Sink,
    ) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>,
    {
        let records = schema
            .record
            .members()
            .iter()
            .map(|member| {
                format!(
                    "{}@{}+{}",
                    member.path(),
                    member.absolute_offset(),
                    member.range().len()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let registers = schema
            .map
            .registers()
            .iter()
            .map(|register| format!("{}@{}", register.path(), register.offset()))
            .collect::<Vec<_>>()
            .join(",");
        sink.emit(Artifact {
            logical_name: String::from("acme-telemetry-layout.txt"),
            media_type: String::from("text/vnd.acme.telemetry-layout"),
            contents: format!(
                "record_size={};record={records};map_size={};registers={registers}",
                schema.record.size(),
                schema.map.size()
            )
            .into_bytes(),
        })
    }
}

/// Returns the stable terminal report derived from the custom artifact and schema locations.
#[must_use]
pub fn output_line() -> Option<String> {
    let mut sink = TelemetrySink::default();
    export_resolved(&mut sink)?;
    let artifact = sink.artifacts().first()?;
    let content = core::str::from_utf8(&artifact.contents).ok()?;
    let fixed = fixed_offset(TelemetryMap::locate(4))?;
    let repeated0 = repeated_offset(TelemetryMap::locate(32), 0)?;
    let repeated1 = repeated_offset(TelemetryMap::locate(64), 1)?;
    Some(format!(
        "artifact={};media={};content={content};flat_record_size={};fixed={fixed};repeated0={repeated0};repeated1={repeated1}",
        artifact.logical_name,
        artifact.media_type,
        TelemetryRecord::SPEC.size(),
    ))
}

/// Exercises all five composable macros and returns their literal downstream observables.
#[must_use]
pub fn composable_output_line() -> Option<String> {
    let mut nested = [0_u8; 16];
    let mut pair = DownPairViewMut::new(&mut nested).ok()?;
    pair.left_mut().ok()?.set_sequence(0x1122_3344).ok()?;
    pair.right_mut().ok()?.set_flags(0x5566).ok()?;
    let left = DownPairView::new(&nested).ok()?.left().ok()?.sequence().ok()?;

    let mut registers = [0_u8; 32];
    let mut map = DownMapViewMut::new(&mut registers).ok()?;
    map.control_mut().ok()?.set_enable(1).ok()?;
    map.lane_status_mut(0).ok()?.set_value(0x1234).ok()?;
    let map = DownMapView::new(&registers).ok()?;
    let enable = map.control().ok()?.enable().ok()?;
    let status = map.lane_status(0).ok()?.value().ok()?;
    Some(format!(
        "nested={nested:02x?};le={:02x?};be={:02x?};left={left:#010x};enable={enable};status={status:#06x}",
        &registers[..4],
        &registers[16..18],
    ))
}

const fn fixed_offset(location: Option<RegisterLocation<TelemetryMapRegisterId>>) -> Option<usize> {
    match location {
        Some(RegisterLocation::Fixed { offset, .. }) => Some(offset),
        Some(RegisterLocation::Repeated { .. }) | None => None,
    }
}

const fn repeated_offset(
    location: Option<RegisterLocation<TelemetryMapRegisterId>>,
    expected_instance: usize,
) -> Option<usize> {
    match location {
        Some(RegisterLocation::Repeated {
            instance, offset, ..
        }) if instance == expected_instance => Some(offset),
        Some(RegisterLocation::Fixed { .. } | RegisterLocation::Repeated { .. }) | None => None,
    }
}
