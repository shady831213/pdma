use super::PdmaRegisters;
use pdma_model_contract::abi::mmio::{MmioAccess, PdmaMmioMap, decode_read};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn typed_values(registers: &PdmaRegisters) -> TestResult<[u64; 66]> {
    registers.view(|root| {
        Ok([
            root.g_magic()?.value()?,
            root.g_mmio_abi_version()?.value()?,
            root.g_req_abi_version()?.value()?,
            root.g_req_features()?.value()?,
            root.g_reset_expected_epoch()?.value()?,
            root.g_command()?.value()?,
            root.g_result()?.value()?,
            root.g_detail0()?.value()?,
            root.g_detail1()?.value()?,
            root.g_detail2()?.value()?,
            root.g_rsp_abi_version()?.value()?,
            root.g_rsp_run_state()?.value()?,
            root.g_rsp_features()?.value()?,
            root.g_rsp_epoch()?.value()?,
            root.g_max_ring_depth()?.value()?,
            root.g_max_indirect_destinations()?.value()?,
            root.g_max_transfer_bytes()?.value()?,
            root.g_max_strided_rows()?.value()?,
            root.g_max_stride()?.value()?,
            root.q_expected_epoch()?.value()?,
            root.q_id()?.value()?,
            root.q_expected_cursor()?.value()?,
            root.q_new_cursor()?.value()?,
            root.q_cfg_sq_depth()?.value()?,
            root.q_cfg_cq_depth()?.value()?,
            root.q_cfg_event_policy()?.value()?,
            root.q_cfg_software_owner()?.value()?,
            root.q_sq_base()?.value()?,
            root.q_sq_depth()?.value()?,
            root.q_cq_base()?.value()?,
            root.q_cq_depth()?.value()?,
            root.q_eq_base()?.value()?,
            root.q_eq_depth()?.value()?,
            root.q_command()?.value()?,
            root.q_result()?.value()?,
            root.q_detail0()?.value()?,
            root.q_detail1()?.value()?,
            root.q_detail2()?.value()?,
            root.q_rsp_epoch()?.value()?,
            root.q_rsp_id()?.value()?,
            root.q_rsp_sq_producer()?.value()?,
            root.q_rsp_sq_captured()?.value()?,
            root.q_rsp_sq_consumer()?.value()?,
            root.q_rsp_cq_producer()?.value()?,
            root.q_rsp_cq_consumer()?.value()?,
            root.q_rsp_eq_producer()?.value()?,
            root.q_rsp_eq_consumer()?.value()?,
            root.i_expected_epoch()?.value()?,
            root.i_source_id()?.value()?,
            root.i_source_kind()?.value()?,
            root.i_route_kind()?.value()?,
            root.i_route_vector()?.value()?,
            root.i_command()?.value()?,
            root.i_result()?.value()?,
            root.i_detail0()?.value()?,
            root.i_detail1()?.value()?,
            root.i_detail2()?.value()?,
            root.i_rsp_epoch()?.value()?,
            root.i_rsp_source_id()?.value()?,
            root.i_rsp_source_kind()?.value()?,
            root.i_rsp_state()?.value()?,
            root.i_rsp_route_kind()?.value()?,
            root.i_rsp_notification_kind()?.value()?,
            root.i_rsp_vector()?.value()?,
            root.i_rsp_message_id()?.value()?,
            root.i_rsp_pending_messages()?.value()?,
        ])
    })
}
fn write_typed_requests(registers: &PdmaRegisters) -> TestResult {
    registers.view_mut(|mut root| -> TestResult {
        root.g_req_abi_version_mut()?.set_value(2)?;
        root.g_req_features_mut()?.set_value(2)?;
        root.g_reset_expected_epoch_mut()?.set_value(2)?;
        root.q_expected_epoch_mut()?.set_value(2)?;
        root.q_id_mut()?.set_value(2)?;
        root.q_expected_cursor_mut()?.set_value(2)?;
        root.q_new_cursor_mut()?.set_value(2)?;
        root.q_cfg_sq_depth_mut()?.set_value(2)?;
        root.q_cfg_cq_depth_mut()?.set_value(2)?;
        root.q_cfg_event_policy_mut()?.set_value(0)?;
        root.q_cfg_software_owner_mut()?.set_value(2)?;
        root.q_sq_base_mut()?.set_value(2)?;
        root.q_sq_depth_mut()?.set_value(2)?;
        root.q_cq_base_mut()?.set_value(2)?;
        root.q_cq_depth_mut()?.set_value(2)?;
        root.q_eq_base_mut()?.set_value(2)?;
        root.q_eq_depth_mut()?.set_value(2)?;
        root.i_expected_epoch_mut()?.set_value(2)?;
        root.i_source_id_mut()?.set_value(2)?;
        root.i_source_kind_mut()?.set_value(1)?;
        root.i_route_kind_mut()?.set_value(1)?;
        root.i_route_vector_mut()?.set_value(2)?;
        Ok(())
    })
}

#[test]
fn every_typed_leaf_observes_raw_requests() -> TestResult {
    // Given: the same 66-leaf image used by the endpoint.
    let registers = PdmaRegisters::new();
    // When: raw writes set every writable request to its lowest permitted bit.
    for register in PdmaMmioMap::resolve()?.registers() {
        let definition = register.definition();
        if definition.access() == MmioAccess::ReadWrite {
            let value = definition.writable_mask() & 1;
            let bytes = value.to_le_bytes();
            registers.write(
                register.id(),
                bytes.get(..definition.width()).ok_or("width")?,
            );
        }
    }
    // Then: all typed leaves, including command and response leaves, retain exact values.
    assert_eq!(
        typed_values(&registers)?,
        [
            0x414d_4450,
            1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            1,
            1,
            1,
            1,
            1,
            0,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
        ]
    );
    Ok(())
}

#[test]
fn every_typed_request_updates_raw_bytes() -> TestResult {
    // Given: a fresh register image.
    let registers = PdmaRegisters::new();
    // When: every request is changed through its typed mutable accessor.
    write_typed_requests(&registers)?;
    // Then: raw leaf bytes contain the independently specified values.
    for (offset, width, value) in [
        (0x008, 4, 2),
        (0x010, 8, 2),
        (0x018, 8, 2),
        (0x100, 8, 2),
        (0x108, 8, 2),
        (0x110, 8, 2),
        (0x118, 8, 2),
        (0x120, 4, 2),
        (0x124, 4, 2),
        (0x128, 4, 0),
        (0x130, 8, 2),
        (0x138, 8, 2),
        (0x140, 4, 2),
        (0x148, 8, 2),
        (0x150, 4, 2),
        (0x158, 8, 2),
        (0x160, 4, 2),
        (0x200, 8, 2),
        (0x208, 8, 2),
        (0x210, 4, 1),
        (0x214, 4, 1),
        (0x218, 8, 2),
    ] {
        let register = decode_read(offset, width)?;
        let bytes = u64::to_le_bytes(value);
        assert_eq!(
            registers.read(register),
            bytes.get(..usize::try_from(width)?).ok_or("width")?
        );
    }
    Ok(())
}
