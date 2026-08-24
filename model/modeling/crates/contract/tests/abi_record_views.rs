//! Public generated record-view behavior.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::abi::{
    CompletionSchemaView, CompletionSchemaViewMut, DescriptorSchemaView, DescriptorSchemaViewMut,
    EventSchemaView, EventSchemaViewMut, IndirectDestinationEntrySchemaView,
    IndirectDestinationEntrySchemaViewMut,
};

#[test]
fn generated_views_when_every_field_is_written_preserve_exact_values()
-> Result<(), SchemaCodecError> {
    let mut descriptor = [0_u8; 64];
    let mut completion = [0_u8; 32];
    let mut event = [0_u8; 64];
    let mut indirect = [0_u8; 8];

    write_descriptor(&mut descriptor)?;
    let mut completion_mut = CompletionSchemaViewMut::new(&mut completion).unwrap();
    completion_mut.set_version(1).unwrap();
    completion_mut.set_size(32).unwrap();
    completion_mut.set_status(3).unwrap();
    completion_mut.set_flags(4).unwrap();
    completion_mut.set_origin_queue_id(5).unwrap();
    completion_mut.set_command_id(6).unwrap();
    completion_mut.set_reserved(7).unwrap();
    let mut event_mut = EventSchemaViewMut::new(&mut event).unwrap();
    event_mut.set_version(1).unwrap();
    event_mut.set_size(64).unwrap();
    event_mut.set_kind(3).unwrap();
    event_mut.set_flags(4).unwrap();
    event_mut.set_origin_queue_id(5).unwrap();
    event_mut.set_command_id(6).unwrap();
    event_mut.set_fault_locator(7).unwrap();
    event_mut.set_detail0(8).unwrap();
    event_mut.set_detail1(9).unwrap();
    event_mut.set_stage(10).unwrap();
    event_mut.set_access(11).unwrap();
    event_mut.set_reserved(12).unwrap();
    event_mut.set_index(13).unwrap();
    IndirectDestinationEntrySchemaViewMut::new(&mut indirect)
        .unwrap()
        .set_destination_iova(14)
        .unwrap();

    assert_descriptor(&descriptor)?;
    let completion = CompletionSchemaView::new(&completion).unwrap();
    assert_eq!(
        (
            completion.version().unwrap(),
            completion.size().unwrap(),
            completion.status().unwrap(),
            completion.flags().unwrap(),
            completion.origin_queue_id().unwrap(),
            completion.command_id().unwrap(),
            completion.reserved().unwrap(),
        ),
        (1, 32, 3, 4, 5, 6, 7)
    );
    let event = EventSchemaView::new(&event).unwrap();
    assert_eq!(
        (
            event.version().unwrap(),
            event.size().unwrap(),
            event.kind().unwrap(),
            event.flags().unwrap(),
            event.origin_queue_id().unwrap(),
            event.command_id().unwrap(),
            event.fault_locator().unwrap(),
            event.detail0().unwrap(),
            event.detail1().unwrap(),
            event.stage().unwrap(),
            event.access().unwrap(),
            event.reserved().unwrap(),
        ),
        (1, 64, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)
    );
    assert_eq!(event.index(), Ok(13));
    assert_eq!(
        IndirectDestinationEntrySchemaView::new(&indirect)
            .unwrap()
            .destination_iova(),
        Ok(14)
    );
    Ok(())
}

fn write_descriptor(bytes: &mut [u8; 64]) -> Result<(), SchemaCodecError> {
    let mut descriptor = DescriptorSchemaViewMut::new(bytes)?;
    {
        let mut header = descriptor.header_mut()?;
        header.set_version(1)?;
        header.set_opcode(2)?;
        header.set_flags(3)?;
        header.set_size(64)?;
        header.set_command_id(5)?;
    }
    descriptor.set_word0(6)?;
    descriptor.set_word1(7)?;
    descriptor.set_word2(8)?;
    descriptor.set_word3(9)?;
    descriptor.set_word4(10)?;
    descriptor.set_word5(11)?;
    Ok(())
}

fn assert_descriptor(bytes: &[u8; 64]) -> Result<(), SchemaCodecError> {
    let descriptor = DescriptorSchemaView::new(bytes)?;
    let header = descriptor.header()?;
    assert_eq!(
        (
            header.version()?,
            header.opcode()?,
            header.flags()?,
            header.size()?,
            header.command_id()?,
            descriptor.word0()?,
            descriptor.word1()?,
            descriptor.word2()?,
            descriptor.word3()?,
            descriptor.word4()?,
            descriptor.word5()?,
        ),
        (1, 2, 3, 64, 5, 6, 7, 8, 9, 10, 11)
    );
    Ok(())
}

#[test]
fn generated_mutable_views_when_writing_every_record_are_public_and_atomic() {
    let mut descriptor = [0xa5_u8; 64];
    let mut completion = [0xa5_u8; 32];
    let mut event = [0xa5_u8; 64];
    let mut indirect = [0xa5_u8; 8];

    let descriptor_before = descriptor;
    let completion_before = completion;
    let event_before = event;
    let indirect_before = indirect;

    assert!(DescriptorSchemaViewMut::new(&mut descriptor[..63]).is_err());
    assert!(CompletionSchemaViewMut::new(&mut completion[..31]).is_err());
    assert!(EventSchemaViewMut::new(&mut event[..63]).is_err());
    assert!(IndirectDestinationEntrySchemaViewMut::new(&mut indirect[..7]).is_err());

    assert_eq!(descriptor, descriptor_before);
    assert_eq!(completion, completion_before);
    assert_eq!(event, event_before);
    assert_eq!(indirect, indirect_before);
}
