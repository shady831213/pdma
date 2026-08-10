//! A standalone downstream consumer of the schema facade.

use core::convert::Infallible;

use his::{
    export::{Artifact, ArtifactSink, Exporter},
    record_schema, register_map,
    register_map::RegisterLocation,
    register_schema,
};

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

impl Exporter<his::register_map::RegisterMapSpec<TelemetryMapRegisterId>> for TelemetryExporter {
    type Error = Infallible;

    fn export<Sink>(
        &self,
        _schema: &his::register_map::RegisterMapSpec<TelemetryMapRegisterId>,
        sink: &mut Sink,
    ) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>,
    {
        sink.emit(Artifact {
            logical_name: String::from("acme-telemetry-layout.txt"),
            media_type: String::from("text/vnd.acme.telemetry-layout"),
            contents: b"record=12,map=128".to_vec(),
        })
    }
}

/// Returns the stable terminal report derived from the custom artifact and schema locations.
#[must_use]
pub fn output_line() -> Option<String> {
    let exporter = TelemetryExporter;
    let mut sink = TelemetrySink::default();
    exporter.export(&TelemetryMap::SPEC, &mut sink).ok()?;
    let artifact = sink.artifacts().first()?;
    let content = core::str::from_utf8(&artifact.contents).ok()?;
    let fixed = fixed_offset(TelemetryMap::locate(4))?;
    let repeated0 = repeated_offset(TelemetryMap::locate(32), 0)?;
    let repeated1 = repeated_offset(TelemetryMap::locate(64), 1)?;
    Some(format!(
        "artifact={};media={};content={content};record_size={};map_size={};fixed={fixed};repeated0={repeated0};repeated1={repeated1}",
        artifact.logical_name,
        artifact.media_type,
        TelemetryRecord::SPEC.size(),
        TelemetryMap::SPEC.size(),
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
