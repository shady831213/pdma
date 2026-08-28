//! A standalone downstream consumer of the schema facade.

use core::convert::Infallible;

use his::{
    bits_schema,
    export::{Artifact, ArtifactSink, Exporter},
    record_schema, register_map,
    register_map::RegisterLocation,
    register_schema,
};

bits_schema! { pub bits DownControlBits(u8) { enable: 0..1, mode: 1..3, } }
record_schema! { pub record DownControlRecord { endian: Little, fields: {
    flags: bits(DownControlBits),
} } }
record_schema! { pub record DownHeader {
    alignment: 4, endian: Little,
    fields: {
        sequence: u32,
        flags: u16,
    },
} }
record_schema! { pub record DownPair {
    alignment: 8, endian: Little,
    fields: {
        left: DownHeader,
        right: DownHeader,
    },
} }
register_schema! { pub register DownControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 7,
    fields: DownControlBits,
} }
register_schema! { pub register DownStatus {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff,
    fields: {},
} }
register_map! { pub map DownLaneMap {
    alignment: 2,
    placements: [status: DownStatus],
} }
register_map! { pub map DownMap {
    alignment: 4,
    extent: 32,
    placements: [
        control: DownControl,
        lane: DownLaneMap { offset: 16 },
    ],
} }

record_schema! { pub record TelemetrySamples {
    endian: Little,
    fields: {
        sample0: u16,
        sample1: u16,
    },
} }

record_schema! {
    pub record TelemetryRecord {
        alignment: 4,
        endian: Little,
        fields: {
            sequence: u32,
            temperature: u16,
            flags: u8,
            samples: TelemetrySamples,
        }
    }
}

register_schema! {
    pub register TelemetryController {
        width: 4, endian: Little, access: ReadWrite,
        reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
    }
}

register_schema! {
    pub register TelemetryStatus {
        width: 4, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0, fields: {},
    }
}
register_schema! {
    pub register TelemetryPublish {
        width: 4, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 0xffff_ffff, fields: {},
    }
}
register_schema! {
    pub register TelemetryFault {
        width: 4, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0, fields: {},
    }
}
register_map! {
    pub map TelemetryLaneMap {
        alignment: 4,
        extent: 32,
        placements: [
            publish: TelemetryPublish,
            fault: TelemetryFault { offset: 8 },
        ],
    }
}
register_map! {
    pub map TelemetryMap {
        alignment: 4,
        extent: 128,
        placements: [
            controller: TelemetryController { offset: 4 },
            status: TelemetryStatus { offset: 12 },
            lane: TelemetryLaneMap { offset: 32, count: 2, stride: 32 },
        ],
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
        map: TelemetryMap::resolve().ok()?,
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
            .map(|register| {
                let path = register
                    .path()
                    .strip_prefix("TelemetryMap.")
                    .unwrap_or(register.path());
                format!("{path}@{}", register.offset())
            })
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
    let flags = DownControlBitsValues { enable: true, mode: 2 };
    map.control_mut().ok()?.set_fields(flags).ok()?;
    map.lane_mut().ok()?.status_mut().ok()?.set_value(0x1234).ok()?;
    let map = DownMapView::new(&registers).ok()?;
    let control = map.control().ok()?.fields().ok()?;
    let bits = DownControlRecord { flags }.to_bytes().ok()?;
    let status = map.lane().ok()?.status().ok()?.value().ok()?;
    Some(format!(
        "nested={nested:02x?};le={:02x?};be={:02x?};bits={bits:02x?};left={left:#010x};enable={};mode={};status={status:#06x}",
        &registers[..4],
        &registers[16..18],
        control.enable,
        control.mode,
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
