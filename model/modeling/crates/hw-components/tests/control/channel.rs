//! Bootstrap ownership and independent control-doorbell boundary tests.

use hardware_interface_schema::RegisterAccessError;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::control::{ControlChannel, ControlError};

const BASE: usize = 0x100;
const ENABLE: usize = BASE + 8;
const REQUEST: usize = BASE + 20;
const RESPONSE: usize = BASE + 24;

#[test]
fn bootstrap_and_enable_are_visible_to_both_domains() {
    // Given: hardware with no memory capability or descriptor validation.
    for address in [0_u64, 1, u64::MAX] {
        let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
        // When: Host publishes any bootstrap address and enables it.
        channel
            .host_write(FunctionId::new(0), BASE, &address.to_le_bytes())
            .unwrap();
        channel
            .host_write(FunctionId::new(0), ENABLE, &1_u32.to_le_bytes())
            .unwrap();
        // Then: both domains observe the bootstrap and activation unchanged.
        let mut bootstrap = [0; 8];
        let mut enabled = [0; 4];
        channel
            .host_read(FunctionId::new(0), BASE, &mut bootstrap)
            .unwrap();
        assert_eq!(u64::from_le_bytes(bootstrap), address);
        channel.firmware_read(BASE, &mut bootstrap).unwrap();
        assert_eq!(u64::from_le_bytes(bootstrap), address);
        channel
            .host_read(FunctionId::new(0), ENABLE, &mut enabled)
            .unwrap();
        assert_eq!(enabled, 1_u32.to_le_bytes());
        channel.firmware_read(ENABLE, &mut enabled).unwrap();
        assert_eq!(enabled, 1_u32.to_le_bytes());
    }
}

#[test]
fn each_domain_rejects_wrong_owner_writes_even_when_unchanged() {
    // Given: reset control hardware.
    let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
    // When: domains write each other's registers, including unchanged zero.
    let results = [
        channel.firmware_write(BASE, &[0; 8]),
        channel.firmware_write(ENABLE, &[0; 4]),
        channel.firmware_write(REQUEST, &[0; 4]),
        channel.host_write(FunctionId::new(0), RESPONSE, &[0; 4]),
        channel.firmware_write(REQUEST, &1_u32.to_le_bytes()),
        channel.host_write(FunctionId::new(0), RESPONSE, &1_u32.to_le_bytes()),
    ];
    // Then: every wrong-owner write rejects and no pulse escapes.
    assert!(
        results
            .into_iter()
            .all(|result| result == Err(RegisterAccessError::Other(ControlError::Permission)))
    );
    assert!(!channel.take_request_notification());
    assert!(!channel.take_response_notification());
}

#[test]
fn activation_locks_bootstrap_changes_and_disable() {
    // Given: enabled hardware, with bootstrap zero permitted.
    let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
    channel
        .host_write(FunctionId::new(0), ENABLE, &1_u32.to_le_bytes())
        .unwrap();
    // When: Host changes the bootstrap or disables the channel.
    let changes = [
        channel.host_write(FunctionId::new(0), BASE, &8_u64.to_le_bytes()),
        channel.host_write(FunctionId::new(0), ENABLE, &[0; 4]),
    ];
    // Then: configuration stays locked; repeated unchanged configuration is legal.
    assert!(changes.into_iter().all(|result| result
        == Err(RegisterAccessError::Other(
            ControlError::ConfigurationLocked
        ))));
    channel
        .host_write(FunctionId::new(0), BASE, &[0; 8])
        .unwrap();
    channel
        .host_write(FunctionId::new(0), ENABLE, &1_u32.to_le_bytes())
        .unwrap();
    let mut bytes = [0xff; 8];
    channel.firmware_read(BASE, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 8]);
}

#[test]
fn owned_doorbells_coalesce_and_take_clears_only_its_direction() {
    // Given: hardware accepts notifications independently of activation.
    let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
    assert!(!channel.take_request_notification());
    assert!(!channel.take_response_notification());
    // When: both owners ring repeatedly before the adapter drains pulses.
    for _ in 0..3 {
        channel
            .host_write(FunctionId::new(0), REQUEST, &1_u32.to_le_bytes())
            .unwrap();
        channel
            .firmware_write(RESPONSE, &1_u32.to_le_bytes())
            .unwrap();
    }
    // Then: each direction yields exactly one pulse and can be rung again.
    assert!(channel.take_request_notification());
    assert!(!channel.take_request_notification());
    assert!(channel.take_response_notification());
    assert!(!channel.take_response_notification());
    channel
        .host_write(FunctionId::new(0), REQUEST, &1_u32.to_le_bytes())
        .unwrap();
    channel
        .firmware_write(RESPONSE, &1_u32.to_le_bytes())
        .unwrap();
    assert!(channel.take_request_notification());
    assert!(channel.take_response_notification());
}

#[test]
fn drained_doorbells_do_not_replay_on_unrelated_register_writes() {
    // Given: both doorbells have emitted a pulse that the adapter drained.
    let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
    channel
        .host_write(FunctionId::new(0), REQUEST, &1_u32.to_le_bytes())
        .unwrap();
    channel
        .firmware_write(RESPONSE, &1_u32.to_le_bytes())
        .unwrap();
    assert!(channel.take_request_notification());
    assert!(channel.take_response_notification());
    // When: Host publishes a bootstrap and enables the channel.
    channel
        .host_write(FunctionId::new(0), BASE, &8_u64.to_le_bytes())
        .unwrap();
    channel
        .host_write(FunctionId::new(0), ENABLE, &1_u32.to_le_bytes())
        .unwrap();
    // Then: cleared doorbell values cannot replay either notification.
    assert!(!channel.take_request_notification());
    assert!(!channel.take_response_notification());
}

#[test]
fn zero_doorbell_writes_neither_create_nor_clear_pending_pulses() {
    // Given: both idle and pending directions.
    for pending in [false, true] {
        let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
        if pending {
            channel
                .host_write(FunctionId::new(0), REQUEST, &1_u32.to_le_bytes())
                .unwrap();
            channel
                .firmware_write(RESPONSE, &1_u32.to_le_bytes())
                .unwrap();
        }
        // When: each owner writes zero.
        channel
            .host_write(FunctionId::new(0), REQUEST, &[0; 4])
            .unwrap();
        channel.firmware_write(RESPONSE, &[0; 4]).unwrap();
        // Then: zero has no effect on pending pulses.
        assert_eq!(channel.take_request_notification(), pending);
        assert_eq!(channel.take_response_notification(), pending);
    }
}

#[test]
fn malformed_doorbell_writes_do_not_emit_pulses() {
    // Given: idle hardware and reserved-bit, wrong-width and partial writes.
    for bytes in [&[3, 0, 0, 0][..], &[1, 0, 0], &[1, 0, 0, 0, 0, 0, 0, 0]] {
        let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
        // When: malformed values reach each owned doorbell.
        let results = [
            channel.host_write(FunctionId::new(0), REQUEST, bytes),
            channel.firmware_write(RESPONSE, bytes),
        ];
        // Then: validation rejects before either pulse is emitted.
        assert!(results.into_iter().all(|result| result.is_err()));
        assert!(!channel.take_request_notification());
        assert!(!channel.take_response_notification());
    }
}

#[test]
fn doorbell_reads_are_forbidden_for_both_domains_without_consuming_pulses() {
    // Given: pending notifications in both directions.
    let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
    channel
        .host_write(FunctionId::new(0), REQUEST, &1_u32.to_le_bytes())
        .unwrap();
    channel
        .firmware_write(RESPONSE, &1_u32.to_le_bytes())
        .unwrap();
    // When: either domain reads the write-only leaves.
    for address in [REQUEST, RESPONSE] {
        let mut output = [0xa5; 4];
        assert_eq!(
            channel.host_read(FunctionId::new(0), address, &mut output),
            Err(RegisterAccessError::ReadForbidden {
                offset: address - BASE
            })
        );
        assert_eq!(
            channel.firmware_read(address, &mut output),
            Err(RegisterAccessError::ReadForbidden {
                offset: address - BASE
            })
        );
        // Then: output and the pending notifications remain untouched.
        assert_eq!(output, [0xa5; 4]);
    }
    assert!(channel.take_request_notification());
    assert!(channel.take_response_notification());
}

#[test]
fn malformed_access_preserves_state_and_read_output() {
    // Given: reset hardware.
    let mut channel = ControlChannel::new(BASE, FunctionId::new(0));
    let mut output = [0xa5; 3];
    // When: callers use wrong widths, reserved bits, or unmapped addresses.
    assert!(
        channel
            .host_read(FunctionId::new(0), ENABLE, &mut output)
            .is_err()
    );
    assert!(
        channel
            .host_write(FunctionId::new(0), ENABLE, &2_u32.to_le_bytes())
            .is_err()
    );
    assert!(
        channel
            .host_write(FunctionId::new(0), BASE, &[0; 4])
            .is_err()
    );
    assert!(
        channel
            .host_read(FunctionId::new(0), BASE + 1, &mut [0; 4])
            .is_err()
    );
    assert!(
        channel
            .host_read(FunctionId::new(0), BASE - 1, &mut [0; 4])
            .is_err()
    );
    assert!(
        channel
            .host_read(
                FunctionId::new(0),
                BASE + pdma_model_contract::hw_components::control::ControlRegisters::EXTENT,
                &mut [0; 4]
            )
            .is_err()
    );
    // Then: failed reads preserve the destination and enable remains clear.
    assert_eq!(output, [0xa5; 3]);
    let mut enabled = [0xff; 4];
    channel
        .host_read(FunctionId::new(0), ENABLE, &mut enabled)
        .unwrap();
    assert_eq!(enabled, [0; 4]);
}
