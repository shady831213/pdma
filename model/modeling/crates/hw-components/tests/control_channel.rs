//! Control-channel register permission and pointer boundary tests.

use hardware_interface_schema::RegisterAccessError;
use pdma_model_contract::RingError;
use pdma_model_hw_components::{ControlChannel, ControlError};

const BASE: usize = 0x100;
const ENABLE: usize = BASE + 44;

fn configured(capacity: u32) -> Result<ControlChannel, RegisterAccessError<ControlError>> {
    let mut channel = ControlChannel::new(BASE);
    for (offset, address) in [(0, 0x1000_u64), (24, 0x2000)] {
        channel.host_write(BASE + offset, &address.to_le_bytes())?;
        channel.host_write(BASE + offset + 8, &capacity.to_le_bytes())?;
    }
    Ok(channel)
}

fn active(capacity: u32) -> Result<ControlChannel, RegisterAccessError<ControlError>> {
    let mut channel = configured(capacity)?;
    channel.host_write(ENABLE, &1_u32.to_le_bytes())?;
    Ok(channel)
}

fn read(
    channel: &mut ControlChannel,
    offset: usize,
) -> Result<u32, RegisterAccessError<ControlError>> {
    let mut bytes = [0; 4];
    channel.host_read(BASE + offset, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

#[test]
fn enable_rejects_invalid_geometry_without_publishing_enable() {
    // Given: either ring has invalid capacity, alignment, or overflowing extent.
    for offset in [0, 24] {
        for (base, capacity) in [
            (0x1000_u64, 0_u32),
            (0x1000, 3),
            (0x1000, 1 << 31),
            (0x1001, 2),
            (u64::MAX - 7, 2),
        ] {
            let mut channel = configured(2).unwrap();
            channel
                .host_write(BASE + offset, &base.to_le_bytes())
                .unwrap();
            channel
                .host_write(BASE + offset + 8, &capacity.to_le_bytes())
                .unwrap();
            // When: Host activates the rings.
            let result = channel.host_write(ENABLE, &1_u32.to_le_bytes());
            // Then: invalid geometry rejects activation atomically.
            assert_eq!(
                result,
                Err(RegisterAccessError::Other(ControlError::Ring(
                    RingError::Geometry
                )))
            );
            assert_eq!(read(&mut channel, 44).unwrap(), 0);
        }
    }
}

#[test]
fn each_domain_rejects_wrong_owner_writes_even_when_unchanged() {
    // Given: configured but inactive rings.
    let mut channel = configured(2).unwrap();
    // When: either domain writes a pointer or control owned by the other.
    for offset in [16, 36] {
        assert_eq!(
            channel.host_write(BASE + offset, &[0; 4]),
            Err(RegisterAccessError::Other(ControlError::Permission))
        );
    }
    for offset in [8, 12, 32, 40, 44] {
        assert_eq!(
            channel.firmware_write(BASE + offset, &[0; 4]),
            Err(RegisterAccessError::Other(ControlError::Permission))
        );
    }
    for offset in [0, 24] {
        assert_eq!(
            channel.firmware_write(BASE + offset, &[0; 8]),
            Err(RegisterAccessError::Other(ControlError::Permission))
        );
    }
    // Then: both domains can still read inactive state.
    let mut bytes = [0xff; 4];
    channel.firmware_read(ENABLE, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 4]);
}

#[test]
fn inactive_owned_pointer_writes_are_rejected() {
    // Given: configured inactive rings.
    let mut channel = configured(2).unwrap();
    // When: either domain attempts publication or consumption.
    let results = [
        channel.host_write(BASE + 12, &[0; 4]),
        channel.host_write(BASE + 40, &[0; 4]),
        channel.firmware_write(BASE + 16, &[0; 4]),
        channel.firmware_write(BASE + 36, &[0; 4]),
    ];
    // Then: no pointer write is accepted before activation, including zero.
    assert!(
        results
            .into_iter()
            .all(|result| result == Err(RegisterAccessError::Other(ControlError::Disabled)))
    );
}

#[test]
fn activation_locks_geometry_and_disable_until_subsystem_reset() {
    // Given: active control queues.
    let mut channel = active(2).unwrap();
    // When: Host tries changing configuration or disabling.
    for (offset, bytes) in [
        (0, 0x3000_u64.to_le_bytes().to_vec()),
        (24, 0x4000_u64.to_le_bytes().to_vec()),
        (8, 4_u32.to_le_bytes().to_vec()),
        (32, 4_u32.to_le_bytes().to_vec()),
        (44, 0_u32.to_le_bytes().to_vec()),
    ] {
        assert_eq!(
            channel.host_write(BASE + offset, &bytes),
            Err(RegisterAccessError::Other(
                ControlError::ConfigurationLocked
            ))
        );
    }
    // Then: activation remains in effect.
    assert_eq!(read(&mut channel, 44).unwrap(), 1);
}

#[test]
fn pointer_publication_uses_capacity_and_consumption_limits_without_memory() {
    // Given: active capacity-two queues, without any attached memory service.
    let mut channel = active(2).unwrap();
    // When: software publishes, fills, consumes, and reuses queue slots.
    for (producer, consumer, host_produces) in [(12, 16, true), (36, 40, false)] {
        let mut write = |offset, value: u32, host| {
            if host {
                channel.host_write(BASE + offset, &value.to_le_bytes())
            } else {
                channel.firmware_write(BASE + offset, &value.to_le_bytes())
            }
        };
        let pointers = Err(RegisterAccessError::Other(ControlError::Ring(
            RingError::Pointers,
        )));
        assert_eq!(write(consumer, 1, !host_produces), pointers);
        write(producer, 2, host_produces).unwrap();
        assert_eq!(write(producer, 3, host_produces), pointers);
        assert_eq!(write(producer, 1, host_produces), pointers);
        assert_eq!(write(consumer, 3, !host_produces), pointers);
        write(consumer, 1, !host_produces).unwrap();
        write(producer, 3, host_produces).unwrap();
        write(consumer, 3, !host_produces).unwrap();
        // Then: publications are visible without hardware memory operations.
        assert_eq!(read(&mut channel, producer).unwrap(), 3);
        assert_eq!(read(&mut channel, consumer).unwrap(), 3);
    }
}

#[test]
fn pointer_counts_wrap_without_losing_occupancy() {
    // Given: a large valid ring permits reaching the u32 boundary in four steps.
    let mut channel = active(1 << 30).unwrap();
    for next in [1_u32 << 30, 1 << 31, 3 << 30] {
        channel.host_write(BASE + 12, &next.to_le_bytes()).unwrap();
        channel
            .firmware_write(BASE + 16, &next.to_le_bytes())
            .unwrap();
        channel
            .firmware_write(BASE + 36, &next.to_le_bytes())
            .unwrap();
        channel.host_write(BASE + 40, &next.to_le_bytes()).unwrap();
    }
    // When: each producer and consumer passes the wrapping boundary.
    channel.host_write(BASE + 12, &[0; 4]).unwrap();
    channel.firmware_write(BASE + 16, &[0; 4]).unwrap();
    channel.firmware_write(BASE + 36, &[0; 4]).unwrap();
    channel.host_write(BASE + 40, &[0; 4]).unwrap();
    // Then: both queues are empty at the wrapped position.
    for offset in [12, 16, 36, 40] {
        assert_eq!(read(&mut channel, offset).unwrap(), 0);
    }
    assert_eq!(
        channel.firmware_write(BASE + 16, &1_u32.to_le_bytes()),
        Err(RegisterAccessError::Other(ControlError::Ring(
            RingError::Pointers
        )))
    );
}

#[test]
fn malformed_access_preserves_state_and_read_output() {
    // Given: an inactive channel with valid geometry.
    let mut channel = configured(2).unwrap();
    let mut output = [0xa5; 3];
    // When: callers use wrong widths, reserved bits, padding or unmapped addresses.
    assert!(channel.host_read(ENABLE, &mut output).is_err());
    assert!(channel.host_write(ENABLE, &2_u32.to_le_bytes()).is_err());
    assert!(channel.host_write(BASE + 12, &[0; 8]).is_err());
    assert!(channel.host_read(BASE + 20, &mut [0; 4]).is_err());
    assert!(channel.host_read(BASE - 1, &mut [0; 4]).is_err());
    assert!(channel.host_read(BASE + 48, &mut [0; 4]).is_err());
    // Then: failed reads leave the destination intact and enable remains clear.
    assert_eq!(output, [0xa5; 3]);
    assert_eq!(read(&mut channel, 44).unwrap(), 0);
}
